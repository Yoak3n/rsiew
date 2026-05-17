use serde::{Deserialize, Serialize};

/// 网站语义分类规则
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebsiteSemanticRule {
    /// 域名
    pub domain: String,
    /// 目标语义分类
    pub semantic_category: String,
}


/// 应用分类规则
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppCategoryRule {
    /// 应用名称
    pub app_name: String,
    /// 目标分类
    pub category: String,
}

/// 应用自定义描述规则
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppDescriptionRule {
    /// 应用名称（支持模糊匹配）
    pub app_name: String,
    /// 自定义显示名称（可选，覆盖默认名称）
    #[serde(default)]
    pub display_name: Option<String>,
    /// 自定义描述（显示在应用名称下方）
    #[serde(default)]
    pub description: Option<String>,
    /// 自定义分类（可选，覆盖自动分类）
    #[serde(default)]
    pub category: Option<String>,
}

/// 用户自定义分类
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CustomCategory {
    /// 唯一标识（slug 格式，如 "project-mgmt"）
    pub key: String,
    /// 显示名称（用户输入，如 "项目管理"）
    pub name: String,
    /// 颜色（hex 格式，如 "#8B5CF6"）
    pub color: String,
    /// 图标（emoji，如 "📋"）
    pub icon: String,
}