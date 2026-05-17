pub mod cli;
pub mod core;
pub mod database;
pub mod schema;
pub mod service;
pub mod user_path;
pub mod utils;
pub mod config;
use service::screenshot::ScreenshotService;

use std::sync::{Arc, Mutex};
use schema::AppState;
use core::{init, handle::Handle};
#[cfg(all(windows, not(debug_assertions)))]
use user_path::add_to_user_path;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::new().filter_level(log::LevelFilter::Debug).init();
    #[cfg(all(windows, not(debug_assertions)))]
    add_to_user_path();
    let data_dir = utils::get_data_dir();
    let db_path = data_dir.join("rsiew.db");
    let db = database::Database::new(&db_path).unwrap();
    let config = config::AppConfig::default();
    let privacy_filter = config::privacy::PrivacyFilter::from_config(&config.privacy);
    let screenshot_service = ScreenshotService::new(&data_dir, &config.storage);
    // 从其他地方读取is_recording状态
    let app_state = Arc::new(Mutex::new(AppState {
        db,
        config,
        data_dir,
        privacy_filter,
        screenshot_service,
        is_recording: true, cached_active_window: None,
    }));
    let state_clone = app_state.clone();
    service::register_monitor_sevice(state_clone);

    let mut builder = tauri::Builder::default();
    builder = init::setup_plugins(builder);
    builder
        .setup(|app| {
            Handle::global().init(app.handle().clone());
            core::tray::create_tray_icon(app,false)?;
            Ok(())
        })
        .on_window_event(init::setup_window_event)
        .manage(app_state)
        .invoke_handler(init::generate_handlers())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
