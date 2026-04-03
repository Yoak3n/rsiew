use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::schema::AppState;
use crate::{database, monitor};

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

pub fn register_monitor_sevice(state: Arc<Mutex<AppState>>) {
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

            let state_guard = state.lock().unwrap();
            if active_window.app_name != last_app_name
                || active_window.window_title != last_window_title
            {
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
                            last_db_update_time +=
                                Duration::from_millis((seconds_to_add * 1000) as u64);
                        }
                    }
                }
            }
        }
    });
}
