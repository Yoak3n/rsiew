use serde::{Deserialize, Serialize};
use regex::Regex;



/// 应用隐私级别
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PrivacyLevel {
    /// 完全记录（截图 + 统计）
    #[default]
    Full,
    /// 内容脱敏（只统计时长，不保存截图）
    Anonymized,
    /// 完全忽略（不记录任何信息）
    Ignored,
}

/// 应用隐私规则
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppPrivacyRule {
    /// 应用名称
    pub app_name: String,
    /// 隐私级别（默认为 Full，兼容旧配置）
    #[serde(default)]
    pub level: PrivacyLevel,
}

/// 隐私配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PrivacyConfig {
    /// 应用隐私规则列表
    #[serde(default)]
    pub app_rules: Vec<AppPrivacyRule>,
    /// 排除的窗口标题关键词（触发时使用 Anonymized 级别）
    #[serde(default)]
    pub excluded_keywords: Vec<String>,
    /// URL 域名黑名单（匹配时完全忽略，不记录）
    #[serde(default)]
    pub excluded_domains: Vec<String>,
    /// 已弃用：敏感词过滤始终启用，此字段仅保留反序列化兼容
    #[serde(default = "default_true")]
    pub filter_sensitive: bool,

    // 兼容旧版
    #[serde(default)]
    pub excluded_apps: Vec<String>,
}

fn default_true() -> bool {
    true
}


/// 隐私检查结果
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrivacyAction {
    /// 正常记录（截图 + 统计）
    Record,
    /// 内容脱敏（只统计时长，不保存截图）
    Anonymize,
    /// 完全跳过（不记录任何信息）
    Skip,
}


/// 隐私过滤器
pub struct PrivacyFilter {
    config: PrivacyConfig,
    sensitive_patterns: Vec<Regex>,
}

impl PrivacyFilter {
    /// 从配置创建隐私过滤器
    pub fn from_config(config: &PrivacyConfig) -> Self {
        let sensitive_patterns = vec![
            // 信用卡号
            Regex::new(r"\b\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b").unwrap(),
            // 手机号
            Regex::new(r"\b1[3-9]\d{9}\b").unwrap(),
            // 身份证号
            Regex::new(r"\b\d{17}[\dXx]\b").unwrap(),
            // 邮箱
            Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap(),
            // 密码字段
            Regex::new(r"(?i)(password|密码|pwd|passwd)[\s:：=]+\S+").unwrap(),
            // API Key
            Regex::new(r"(?i)(api[_-]?key|secret|token)[\s:：=]+\S+").unwrap(),
        ];

        Self {
            config: config.clone(),
            sensitive_patterns,
        }
    }

    /// 检查应用和窗口的隐私操作
    /// 返回应该采取的隐私行动
    pub fn check_privacy(&self, app_name: &str, window_title: &str) -> PrivacyAction {
        // 1. 先检查应用级别的隐私规则
        let app_level = self.config.get_app_privacy_level(app_name);

        match app_level {
            PrivacyLevel::Ignored => {
                log::debug!("应用 {app_name} 设置为完全忽略");
                return PrivacyAction::Skip;
            }
            PrivacyLevel::Anonymized => {
                log::debug!("应用 {app_name} 设置为内容脱敏");
                return PrivacyAction::Anonymize;
            }
            PrivacyLevel::Full => {
                // 继续检查窗口标题
            }
        }

        // 2. 检查窗口标题关键词（匹配时使用脱敏模式）
        if self.config.should_anonymize_by_keyword(window_title) {
            log::debug!("窗口标题 {window_title} 匹配敏感关键词，使用脱敏模式");
            return PrivacyAction::Anonymize;
        }

        PrivacyAction::Record
    }

    /// 检查 URL 域名是否在黑名单中
    /// 如果匹配，返回 Skip；否则返回 Record
    pub fn check_url_privacy(&self, url: Option<&str>) -> PrivacyAction {
        if let Some(url) = url {
            if !url.is_empty() {
                let domain = PrivacyConfig::extract_domain(url);

                for excluded in &self.config.excluded_domains {
                    let excluded_domain = PrivacyConfig::extract_domain(excluded);

                    if !domain.is_empty() && !excluded_domain.is_empty() {
                        if PrivacyConfig::domain_matches(&domain, &excluded_domain) {
                            log::debug!("URL 域名 {domain} 匹配黑名单 {excluded_domain}, 跳过记录");
                            return PrivacyAction::Skip;
                        }
                    }
                }
            }
        }
        PrivacyAction::Record
    }

    /// 综合检查：应用 + 窗口标题 + URL
    pub fn check_privacy_full(
        &self,
        app_name: &str,
        window_title: &str,
        browser_url: Option<&str>,
    ) -> PrivacyAction {
        // 1. 先检查应用和窗口标题
        let app_action = self.check_privacy(app_name, window_title);
        if app_action == PrivacyAction::Skip {
            return PrivacyAction::Skip;
        }

        // 2. 检查 URL 域名黑名单
        let url_action = self.check_url_privacy(browser_url);
        if url_action == PrivacyAction::Skip {
            return PrivacyAction::Skip;
        }

        // 3. 返回应用级别的结果（可能是 Record 或 Anonymize）
        app_action
    }

    /// 兼容旧接口：检查是否应该跳过
    pub fn should_skip(&self, app_name: &str, window_title: &str) -> bool {
        self.check_privacy(app_name, window_title) == PrivacyAction::Skip
    }

    /// 过滤OCR文本中的敏感信息
    pub fn filter_text(&self, text: &str) -> String {
        let mut filtered = text.to_string();

        for pattern in &self.sensitive_patterns {
            filtered = pattern.replace_all(&filtered, "[已过滤]").to_string();
        }

        filtered
    }

    /// 更新配置
    pub fn update_config(&mut self, config: &PrivacyConfig) {
        self.config = config.clone();
    }
}


impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            app_rules: vec![
                AppPrivacyRule {
                    app_name: "1Password".to_string(),
                    level: PrivacyLevel::Ignored,
                },
                AppPrivacyRule {
                    app_name: "Bitwarden".to_string(),
                    level: PrivacyLevel::Ignored,
                },
                AppPrivacyRule {
                    app_name: "Keychain".to_string(),
                    level: PrivacyLevel::Ignored,
                },
            ],
            excluded_keywords: vec![
                "bank".to_string(),
                "login".to_string(),
                "password".to_string(),
                "密码".to_string(),
                "银行".to_string(),
                "支付".to_string(),
            ],
            excluded_domains: vec![], // 默认无域名黑名单
            filter_sensitive: true, // 已弃用，保留兼容
            excluded_apps: vec![],
        }
    }
}


impl PrivacyConfig {
    /// 获取应用的隐私级别
    pub fn get_app_privacy_level(&self, app_name: &str) -> PrivacyLevel {
        log::debug!("检查应用 {app_name} 的隐私级别");
        let normalized = app_name.to_lowercase();
        // 先检查新的规则（精确匹配 + 包含匹配）
        for rule in &self.app_rules {
            let rule_normalized = rule.app_name.to_lowercase();
            if normalized == rule_normalized || normalized.contains(&rule_normalized) {
                log::debug!(
                    "应用 {} 匹配规则 {}, 级别: {:?}",
                    app_name,
                    rule.app_name,
                    rule.level
                );
                return rule.level;
            }
        }
        // 兼容旧版 excluded_apps（视为 Ignored）
        for excluded in &self.excluded_apps {
            let excluded_normalized = excluded.to_lowercase();
            if normalized.contains(&excluded_normalized) {
                return PrivacyLevel::Ignored;
            }
        }
        PrivacyLevel::Full
    }

    /// 检查窗口标题是否触发隐私保护
    pub fn should_anonymize_by_keyword(&self, window_title: &str) -> bool {
        let title_lower = window_title.to_lowercase();
        self.excluded_keywords
            .iter()
            .any(|k| title_lower.contains(&k.to_lowercase()))
    }

    /// 从 URL 中提取域名
    pub fn extract_domain(url: &str) -> String {
        let without_protocol = url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        without_protocol
            .split('/')
            .next()
            .unwrap_or("")
            .to_lowercase()
    }

    /// 后缀匹配域名：domain == pattern 或 domain 以 .pattern 结尾
    pub fn domain_matches(domain: &str, pattern: &str) -> bool {
        if domain == pattern {
            return true;
        }
        domain.ends_with(&format!(".{pattern}"))
    }

    /// 迁移旧版 excluded_apps 到 app_rules
    pub fn migrate_legacy_excluded_apps(&mut self) -> bool {
        if self.excluded_apps.is_empty() {
            return false;
        }
        let existing_names: std::collections::HashSet<String> = self
            .app_rules
            .iter()
            .map(|r| r.app_name.to_lowercase())
            .collect();
        let mut migrated = false;
        for app in self.excluded_apps.drain(..) {
            if !existing_names.contains(&app.to_lowercase()) {
                self.app_rules.push(AppPrivacyRule {
                    app_name: app,
                    level: PrivacyLevel::Ignored,
                });
                migrated = true;
            }
        }
        migrated
    }

    /// 收集所有应忽略的应用名（小写），包含 app_rules 和遗留 excluded_apps
    pub fn collect_ignored_app_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .app_rules
            .iter()
            .filter(|rule| rule.level == PrivacyLevel::Ignored)
            .map(|rule| rule.app_name.to_lowercase())
            .collect();
        for app in &self.excluded_apps {
            let lower = app.to_lowercase();
            if !names.contains(&lower) {
                names.push(lower);
            }
        }
        names
    }

    /// 收集所有域名黑名单（已提取域名、已去空）
    pub fn collect_excluded_domains(&self) -> Vec<String> {
        self.excluded_domains
            .iter()
            .map(|d| Self::extract_domain(d))
            .filter(|d| !d.is_empty())
            .collect()
    }
}