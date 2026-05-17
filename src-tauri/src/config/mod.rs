use serde::{Deserialize, Serialize};
pub mod privacy;
pub mod categorize;
pub mod screenshot;
pub mod storage;
pub mod rule;
pub use screenshot::*;
pub use storage::*;
pub use categorize::*;
use privacy::PrivacyConfig;




#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
        /// 隐私配置
    #[serde(default)]
    pub privacy: PrivacyConfig,
    /// 自定义分类
    #[serde(default)]
    pub custom_categories: Vec<CustomCategory>,
    /// 应用分类规则
    #[serde(default)]
    pub app_category_rules: Vec<AppCategoryRule>,
    /// 应用自定义描述规则
    #[serde(default)]
    pub app_description_rules: Vec<AppDescriptionRule>,
    /// 存储配置
    #[serde(default)]
    pub storage: StorageConfig,
}



impl Default for AppConfig {
    fn default() -> Self {
        Self {
            privacy: PrivacyConfig::default(),
            custom_categories: Vec::new(),
            app_category_rules: Vec::new(),
            app_description_rules: Vec::new(),
            storage: StorageConfig::default(),
        }
    }
}

pub fn normalize_category_key_private(value: &str, custom_keys: &[String]) -> String {
    let trimmed = value.trim().to_lowercase();
    match trimmed.as_str() {
        "development" | "browser" | "communication" | "office" | "design" | "entertainment"
        | "other" => trimmed,
        _ if custom_keys.iter().any(|k| k == &trimmed) => trimmed,
        _ => "other".to_string(),
    }
}