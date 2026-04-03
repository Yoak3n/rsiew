mod monitor;
mod ocr;
mod screenlock;
mod screenshot;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{database, schema::AppState, utils::get_data_dir};

fn get_poll_interval_ms() -> u64 {
    #[cfg(target_os = "macos")]
    {
        1500
    }
    #[cfg(not(target_os = "macos"))]
    {
        500
    }
}

fn screen_lock_check_interval_ms() -> u64 {
    #[cfg(target_os = "macos")]
    {
        5000
    }
    #[cfg(not(target_os = "macos"))]
    {
        1000
    }
}

pub fn register_monitor_sevice(state: Arc<Mutex<AppState>>) {
    let cleanup_state_clone = state.clone();

    let screenshot_service = screenshot::ScreenshotService::new(&get_data_dir());
    let ocr_service = ocr::OcrService::new();
    let ocr_semaphore = Arc::new(tokio::sync::Semaphore::new(2));
    let screen_lock_monitor = screenlock::ScreenLockMonitor::new();

    // 定时清理
    tauri::async_runtime::spawn(async move {
        // 每 12 小时检查一次
        let mut interval = tokio::time::interval(Duration::from_secs(12 * 3600));

        loop {
            interval.tick().await;
            // 计算 7 天前的时间戳
            let seven_days_ago = chrono::Local::now().timestamp() - (7 * 24 * 3600);

            println!(
                "🧹 Running cleanup for records older than timestamp: {}",
                seven_days_ago
            );

            let cleanup_guard = cleanup_state_clone.lock().unwrap();
            match cleanup_guard.db.cleanup_old_records(seven_days_ago) {
                Ok(paths) => {
                    let mut deleted_files = 0;
                    // 物理删除硬盘上的截图文件
                    for path in paths {
                        if std::fs::remove_file(&path).is_ok() {
                            deleted_files += 1;
                        }
                    }
                    println!(
                        "✨ Cleanup finished! Deleted {} old screenshots from disk.",
                        deleted_files
                    );
                }
                Err(e) => {
                    println!("❌ Cleanup database error: {}", e);
                }
            }
        }
    });

    // 监听窗口变化
    tauri::async_runtime::spawn(async move {
        let mut last_app_name = String::new();
        let mut last_window_title = String::new();
        let mut last_capture_time = std::time::Instant::now();
        let mut last_db_update_time = std::time::Instant::now();

        let poll_interval_ms = get_poll_interval_ms();

        // 缓存屏幕锁定状态
        let mut last_screen_lock_check = std::time::Instant::now()
            .checked_sub(Duration::from_millis(screen_lock_check_interval_ms()))
            .unwrap_or_else(std::time::Instant::now);
        let mut cached_screen_locked = false;
        loop {
            if last_screen_lock_check.elapsed()
                >= Duration::from_millis(screen_lock_check_interval_ms())
            {
                cached_screen_locked = screen_lock_monitor.is_locked();
                last_screen_lock_check = std::time::Instant::now();
            }

            if cached_screen_locked {
                log::info!("🔒 屏幕已锁定，暂停活动统计");
                last_app_name = String::new(); // 重置应用状态，解锁后视为新开始
                last_capture_time = std::time::Instant::now(); // 重置截图计时，避免解锁后累加锁屏时长
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            // 正式开始
            tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
            
            let active_window = match monitor::get_active_window() {
                Ok(w) => w,
                Err(_) => continue,
            };

            let mut take_screenshot = false;
            let state_guard = state.lock().unwrap();

            if active_window.app_name != last_app_name
                || active_window.window_title != last_window_title
            {
                take_screenshot = true;
                let activity = database::Activity {
                    id: None,
                    timestamp: chrono::Local::now().timestamp(),
                    app_name: active_window.app_name.clone(),
                    window_title: active_window.window_title.clone(),
                    duration: 1,
                    screenshot_path: None,
                    ocr_text: None,
                    exe_path: Some(active_window.exe_path.clone()),
                };

                let activity_id = state_guard.db.insert_activity(&activity).unwrap_or(0);
                last_app_name = active_window.app_name.clone();
                last_window_title = active_window.window_title.clone();
                last_db_update_time = std::time::Instant::now();
                if activity_id > 0 {
                    if let Ok(path) = screenshot_service.capture() {
                        let path_clone = path.clone();
                        // 如果 OCR 提取成功，更新回数据库
                        if let Ok(text) = ocr_service.extract_text(&path_clone) {
                            if !text.is_empty() {
                                let _ = state_guard.db.update_activity_ocr(activity_id, &text);
                            }
                        }
                        let _ = state_guard.db.update_activity_screenshot(
                            activity_id,
                            path.display().to_string().as_str(),
                        );
                    }
                }
            } else {
                let elapsed_since_update = last_db_update_time.elapsed().as_millis() as i64;
                if elapsed_since_update >= 1000 {
                    let seconds_to_add = elapsed_since_update / 1000;
                    if let Ok(Some(latest)) = state_guard.db.get_latest_activity() {
                        if let Some(id) = latest.id {
                            let _ = state_guard.db.merge_activity(id, seconds_to_add);
                            last_db_update_time +=
                                Duration::from_millis((seconds_to_add * 1000) as u64);
                        }
                    }
                }

                let elapsed_since_capture = last_capture_time.elapsed().as_secs() as i64;
                if elapsed_since_capture >= 60 {
                    if let Ok(Some(latest)) = state_guard.db.get_latest_activity() {
                        if let Some(id) = latest.id {
                            if let Ok(path) = screenshot_service.capture() {
                                let path_clone = path.clone();
                                if let Ok(text) = ocr_service.extract_text(&path_clone) {
                                    if !text.is_empty() {
                                        let _ = state_guard.db.update_activity_ocr(id, &text);
                                    }
                                }
                            }
                        }
                    }
                    take_screenshot = true;
                }
            }
            if take_screenshot {
                last_capture_time = std::time::Instant::now();
            }
        }
    });
}


fn should_skip_transient_window(active_window: &monitor::ActiveWindow) -> bool {
    let app_lower = active_window.app_name.to_lowercase();
    matches!(
        app_lower.as_str(),
        "dock"
            | "systemuiserver"
            | "control center"
            | "spotlight"
            | "notificationcenter"
            | "loginwindow"
            | "screencaptureui"
            | "universalaccessauthwarn"
            | "windowmanager"
            | "wallpaper"
    )
}


fn should_skip_system_window(active_window: &monitor::ActiveWindow) -> bool {
    let is_sys = monitor::is_system_process(&active_window.app_name);
    let is_explorer_shell = {
        let name_lower = active_window.app_name.to_lowercase();
        let name_trimmed = name_lower.trim_end_matches(".exe");
        (name_trimmed == "explorer" || name_trimmed == "file explorer")
            && active_window.window_title.is_empty()
    };

    is_sys || is_explorer_shell
}



const ACTIVE_WINDOW_CACHE_MAX_AGE_MS: u64 = 1250;
const MIN_CAPTURE_INTERVAL_MS: u128 = 3000;
const MIN_BROWSER_CHANGE_CAPTURE_INTERVAL_MS: u128 = 1200;

fn reusable_cached_active_window(
    cached: Option<&(std::time::Instant, monitor::ActiveWindow)>,
    now: std::time::Instant,
) -> Option<monitor::ActiveWindow> {
    let (sampled_at, active_window) = cached?;
    let age = now.checked_duration_since(*sampled_at)?;

    if age > Duration::from_millis(ACTIVE_WINDOW_CACHE_MAX_AGE_MS) {
        return None;
    }

    Some(active_window.clone())
}