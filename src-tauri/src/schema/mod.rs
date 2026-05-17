use std::path::PathBuf;

use crate::{config, database, service::{screenshot, monitor}};
use serde::Serialize;

#[derive(Serialize)]
pub struct StatsPayload {
    pub app_name: String,
    pub duration: i64,
    pub exe_path: String,
}

pub struct AppState {
    pub db: database::Database,
    pub data_dir: PathBuf,
    pub config: config::AppConfig,
    pub privacy_filter: config::privacy::PrivacyFilter,
     pub screenshot_service: screenshot::ScreenshotService,
    pub is_recording: bool,
    pub cached_active_window: Option<(std::time::Instant, monitor::ActiveWindow)>,
}
