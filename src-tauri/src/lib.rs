pub mod database;
pub mod monitor;
pub mod ocr;
pub mod screenshot;
pub mod icon_extractor;
pub mod user_path;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::path::PathBuf;
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub struct AppState {
    pub db: database::Database,
}

fn get_data_dir() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("rsiew");
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn get_poll_interval_ms() -> u64 {
    #[cfg(target_os = "macos")] { 1500 }
    #[cfg(not(target_os = "macos"))] { 500 }
}

#[derive(Serialize)]
struct StatsPayload {
    app_name: String,
    duration: i64,
    exe_path: String,
}

fn format_duration(seconds: i64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    if h > 0 {
        format!("{}h {}m", h, m)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len - 1).collect::<String>() + "…"
    }
}

#[tauri::command]
fn get_today_stats(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<Vec<StatsPayload>, String> {
    let state = state.lock().unwrap();
    let end_ts = chrono::Local::now().timestamp();
    let start_ts = end_ts - 86400;

    let usages = state.db.get_stats_by_range(start_ts, end_ts).map_err(|e| e.to_string())?;
    
    let mut payload = Vec::new();
    for u in usages {
        // Since we didn't add exe_path to AppUsage struct, we leave it empty here, 
        // the frontend can use get_app_icon_native with the name if exe_path is missing, 
        // or we could query the latest exe_path from activities.
        payload.push(StatsPayload {
            app_name: u.app_name,
            duration: u.duration,
            exe_path: u.exe_path,
        });
    }
    Ok(payload)
}

#[tauri::command]
fn get_app_icon_native(exe_path: String) -> String {
    icon_extractor::extract_icon_base64(&exe_path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
use tauri_plugin_cli::CliExt;

pub fn run() {
    let data_dir = get_data_dir();
    let db_path = data_dir.join("rsiew.db");
    
    let db = database::Database::new(&db_path).unwrap();
    let _ = db.conn.lock().unwrap().execute("ALTER TABLE activities ADD COLUMN exe_path TEXT", []);
    
    let app_state = Arc::new(Mutex::new(AppState { db }));
    let state_clone = app_state.clone();

    tauri::async_runtime::spawn(async move {
        let mut last_app_name = String::new();
        let mut last_window_title = String::new();
        let mut last_db_update_time = std::time::Instant::now();
        let poll_interval_ms = get_poll_interval_ms();

        loop {
            tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
            let active_window = match monitor::get_active_window() {
                Ok(w) => w,
                Err(_) => continue,
            };

            let state_guard = state_clone.lock().unwrap();
            if active_window.app_name != last_app_name || active_window.window_title != last_window_title {
                let activity = database::Activity {
                    id: None,
                    timestamp: chrono::Local::now().timestamp(),
                    app_name: active_window.app_name.clone(),
                    window_title: active_window.window_title.clone(),
                    duration: 1,
                    screenshot_path: None,
                    ocr_text: None,
                    exe_path: Some(active_window.exe_path.clone())

                };
                
                let _ = state_guard.db.insert_activity(&activity);
                last_app_name = active_window.app_name.clone();
                last_window_title = active_window.window_title.clone();
                last_db_update_time = std::time::Instant::now();
            } else {
                let elapsed_since_update = last_db_update_time.elapsed().as_millis() as i64;
                if elapsed_since_update >= 1000 {
                    let seconds_to_add = elapsed_since_update / 1000;
                    if let Ok(Some(latest)) = state_guard.db.get_latest_activity() {
                        if let Some(id) = latest.id {
                            let _ = state_guard.db.merge_activity(id, seconds_to_add);
                            last_db_update_time += Duration::from_millis((seconds_to_add * 1000) as u64);
                        }
                    }
                }
            }
        }
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_cli::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let is_visible = window.is_visible().unwrap_or(false);
                if !is_visible {
                    let _ = window.show();
                }
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Ok(matches) = app.cli().matches() {
                if let Some(subcommand) = matches.subcommand {
                    match subcommand.name.as_str() {
                        "stats" => {
                            let range = subcommand.matches.args.get("range")
                                .and_then(|a| a.value.as_str())
                                .unwrap_or("today");
                            
                            let start = subcommand.matches.args.get("start")
                                .and_then(|a| {
                                    if let Some(s) = a.value.as_str() {
                                        s.parse::<i64>().ok()
                                    } else {
                                        a.value.as_i64()
                                    }
                                });
                                
                            let end = subcommand.matches.args.get("end")
                                .and_then(|a| {
                                    if let Some(s) = a.value.as_str() {
                                        s.parse::<i64>().ok()
                                    } else {
                                        a.value.as_i64()
                                    }
                                });

                            let now = chrono::Local::now().timestamp();
                            let mut start_ts = 0;
                            let mut end_ts = now;

                            let valid_range = match range {
                                "today" => {
                                    let today = chrono::Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
                                    let today_local = today.and_local_timezone(chrono::Local).unwrap();
                                    start_ts = today_local.timestamp();
                                    true
                                },
                                "week" => {
                                    let today = chrono::Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
                                    let today_local = today.and_local_timezone(chrono::Local).unwrap();
                                    start_ts = today_local.timestamp() - 7 * 24 * 3600;
                                    true
                                },
                                _ => {
                                    println!("Unknown range. Use 'today' or 'week'");
                                    false
                                }
                            };

                            if valid_range {
                                if let Some(s) = start { start_ts = s; }
                                if let Some(e) = end { end_ts = e; }

                                let db_path = get_data_dir().join("rsiew.db");
                                if let Ok(db) = database::Database::new(&db_path) {
                                    if let Ok(stats) = db.get_stats_by_range(start_ts, end_ts) {
                                        let total_seconds: i64 = stats.iter().map(|s| s.duration).sum();

                                        let mut output = String::new();
                                        output.push_str("=========================================================\n");
                                        output.push_str("      Work Dynamics Stats\n");
                                        output.push_str("=========================================================\n");

                                        if stats.is_empty() {
                                            output.push_str("No activity recorded.\n");
                                        } else {
                                            output.push_str(&format!("{:<30} | {:>10} | {:>8}\n", "App Name", "Duration", "%"));
                                            output.push_str("--------------------------------------------------------\n");

                                            for stat in &stats {
                                                let percentage = if total_seconds > 0 {
                                                    (stat.duration as f64 / total_seconds as f64) * 100.0
                                                } else {
                                                    0.0
                                                };
                                                let duration_str = format_duration(stat.duration);
                                                let app_name = truncate_string(&stat.app_name, 30);
                                                output.push_str(&format!(
                                                    "{:<30} | {:>10} | {:>7.1}%\n",
                                                    app_name, duration_str, percentage
                                                ));
                                            }

                                            output.push_str("--------------------------------------------------------\n");
                                            output.push_str(&format!("Total: {:>38}\n", format_duration(total_seconds)));
                                        }
                                        output.push_str("=========================================================\n");
                                        println!("{}", output);
                                    } else {
                                        println!("Error querying stats");
                                    }
                                } else {
                                    println!("Failed to open DB");
                                }
                            }
                            app.handle().exit(0);
                            return Ok(());
                        }
                        "uninstall-cleanup" => {
                            user_path::remove_from_user_path();
                            app.handle().exit(0);
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }

            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            if is_visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![get_today_stats, get_app_icon_native])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}