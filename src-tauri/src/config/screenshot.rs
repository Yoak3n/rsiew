use serde::{Deserialize, Serialize};


/// 存储配置
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ScreenshotDisplayMode {
    /// 仅截活动窗口所在屏幕
    #[default]
    ActiveWindow,
    /// 截取所有屏幕
    All,
}


/// 截图宽度模式
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ScreenshotWidthMode {
    /// 自适应：根据屏幕分辨率动态计算（取屏幕宽度的 70%）
    #[default]
    Auto,
    /// 固定值：使用 max_image_width 设定的值
    Fixed,
}