use tauri::{
    command,State
};
use std::sync::{Arc, Mutex};
use crate::schema::{StatsPayload, AppState};
use crate::icon_extractor::{extract_icon_base64};

#[command]
pub fn get_today_stats(state: State<Arc<Mutex<AppState>>>) -> Result<Vec<StatsPayload>, String> {
    let state = state.lock().unwrap();
    let end_ts = chrono::Local::now().timestamp();
    let start_ts = end_ts - 86400;

    let usages = state.db.get_stats_by_range(start_ts, end_ts).map_err(|e| e.to_string())?;
    
    let mut payload = Vec::new();
    for u in usages {
        payload.push(StatsPayload {
            app_name: u.app_name,
            duration: u.duration,
            exe_path: u.exe_path,
        });
    }
    Ok(payload)
}


#[tauri::command]
pub fn get_app_icon_native(exe_path: String) -> String {
    extract_icon_base64(&exe_path)
}