use serde::Serialize;
use crate::database;

#[derive(Serialize)]
pub struct StatsPayload {
    pub app_name: String,
    pub duration: i64,
    pub exe_path: String,
}


pub struct AppState {
    pub db: database::Database,
}