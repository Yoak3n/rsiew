pub mod cli;
pub mod core;
pub mod database;
pub mod icon_extractor;
pub mod monitor;
pub mod ocr;
pub mod schema;
pub mod screenshot;
pub mod service;
pub mod user_path;
pub mod utils;

use std::sync::{Arc, Mutex};
use schema::AppState;
use core::{init, handle::Handle};


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = utils::get_data_dir();
    let db_path = data_dir.join("rsiew.db");

    let db = database::Database::new(&db_path).unwrap();

    let app_state = Arc::new(Mutex::new(AppState { db }));
    let state_clone = app_state.clone();
    service::register_monitor_sevice(state_clone);

    let mut builder = tauri::Builder::default();
    builder = init::setup_plugins(builder);
    builder
        .setup(|app| {
            Handle::global().init(app.handle().clone());
            core::tray::create_tray_icon(app)?;
            Ok(())
        })
        .on_window_event(init::setup_window_event)
        .manage(app_state)
        .invoke_handler(init::generate_handlers())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
