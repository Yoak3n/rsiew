pub mod extract;
pub mod normalize;
pub mod determine; 
pub mod decode;
pub mod resolve;
pub mod filter;

pub use filter::*;
pub use determine::*;
pub use decode::*;
pub use extract::*;
pub use normalize::*;
pub use resolve::*;



pub fn should_probe_browser_url_before_change_detection(
    app_name: &str,
    window_title: &str,
    last_app_name: Option<&str>,
    last_window_title: Option<&str>,
    current_browser_url: Option<&str>,
) -> bool {
    if !is_browser_app(app_name) || window_title.is_empty() {
        return false;
    }
    // 首次遇到浏览器窗口时（last 为 None），也需要探测 URL
    if last_app_name.is_none() || last_window_title.is_none() {
        return current_browser_url.is_none();
    }
    // 同窗口持续使用时，探测 URL 变化
    last_app_name == Some(app_name) && last_window_title == Some(window_title)
}

const MIN_CAPTURE_INTERVAL_MS: u128 = 3000;
const MIN_BROWSER_CHANGE_CAPTURE_INTERVAL_MS: u128 = 1200;

pub fn browser_change_capture_min_interval_ms(
    app_name: &str,
    title_changed: bool,
    url_changed: bool,
) -> u128 {
    if is_browser_app(app_name) && (title_changed || url_changed) {
        MIN_BROWSER_CHANGE_CAPTURE_INTERVAL_MS
    } else {
        MIN_CAPTURE_INTERVAL_MS
    }
}




