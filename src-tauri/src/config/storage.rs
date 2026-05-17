use serde::{Deserialize, Serialize};
use super::screenshot::{ScreenshotDisplayMode, ScreenshotWidthMode};


/// 存储配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageConfig {
    /// 截图保留天数（超过后删除截图文件）
    pub screenshot_retention_days: u32,
    /// 元数据保留天数（超过后删除数据库记录）
    pub metadata_retention_days: u32,
    /// 存储空间上限（MB），超过后自动清理最旧的数据
    pub storage_limit_mb: u32,
    /// JPEG 质量 (1-100)
    pub jpeg_quality: u8,
    /// 最大图片宽度（超过会缩放）
    pub max_image_width: u32,
    /// 是否启用截图与 OCR
    #[serde(default = "default_screenshots_enabled")]
    pub screenshots_enabled: bool,
    /// 截图屏幕范围
    #[serde(default)]
    pub screenshot_display_mode: ScreenshotDisplayMode,
    /// 截图宽度模式：auto 自适应屏幕，fixed 使用固定值
    #[serde(default)]
    pub screenshot_width_mode: ScreenshotWidthMode,
}

fn default_screenshots_enabled() -> bool {
    true
}


impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            screenshot_retention_days: 7, // 默认保留7天截图
            metadata_retention_days: 30,  // 默认保留30天元数据
            storage_limit_mb: 2048,       // 默认2GB上限
            jpeg_quality: 85,             // 85%质量，更清晰
            max_image_width: 1280,        // 最大宽度1280px
            screenshots_enabled: true,
            screenshot_display_mode: ScreenshotDisplayMode::ActiveWindow,
            screenshot_width_mode: ScreenshotWidthMode::Auto,
        }
    }
}