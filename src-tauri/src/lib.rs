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