use serde::{Deserialize, Serialize};
use crate::config::{AppCategoryRule, AppDescriptionRule};
use crate::utils::get_data_dir;

/// 独立的规则配置，从 rule.json 加载，与 AppConfig 互不影响
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RuleConfig {
    /// 应用分类规则
    #[serde(default)]
    pub app_category_rules: Vec<AppCategoryRule>,
    /// 应用自定义描述规则
    #[serde(default)]
    pub app_description_rules: Vec<AppDescriptionRule>,
}

impl RuleConfig {
    /// 从 rule.json 加载规则配置
    pub fn load() -> Self {
        let rule_path = get_data_dir().join("rule.json");
        if rule_path.exists() {
            match std::fs::read_to_string(&rule_path) {
                Ok(content) => match serde_json::from_str::<RuleConfig>(&content) {
                    Ok(rule) => return rule,
                    Err(e) => eprintln!("警告: rule.json 解析失败: {}", e),
                },
                Err(e) => eprintln!("警告: 读取 rule.json 失败: {}", e),
            }
        }
        RuleConfig::default()
    }

    /// 保存规则配置到 rule.json
    pub fn save(&self) -> Result<(), String> {
        let rule_path = get_data_dir().join("rule.json");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("序列化失败: {}", e))?;
        std::fs::write(&rule_path, json)
            .map_err(|e| format!("写入 {} 失败: {}", rule_path.display(), e))?;
        Ok(())
    }

    // ── 描述规则 CRUD ──

    pub fn add_description(
        &mut self,
        app_name: String,
        display_name: Option<String>,
        description: Option<String>,
    ) -> Result<(), String> {
        if self.app_description_rules.iter().any(|r| r.app_name == app_name) {
            return Err(format!("描述规则 '{}' 已存在，请使用 set-description 更新", app_name));
        }
        self.app_description_rules.push(AppDescriptionRule {
            app_name,
            display_name,
            description,
            category: None,
        });
        self.save()
    }

    pub fn remove_description(&mut self, app_name: &str) -> Result<(), String> {
        let before = self.app_description_rules.len();
        self.app_description_rules.retain(|r| r.app_name != app_name);
        if self.app_description_rules.len() == before {
            return Err(format!("未找到描述规则 '{}'", app_name));
        }
        self.save()
    }

    pub fn update_description(
        &mut self,
        app_name: &str,
        display_name: Option<String>,
        description: Option<String>,
    ) -> Result<(), String> {
        let rule = self.app_description_rules.iter_mut()
            .find(|r| r.app_name == app_name)
            .ok_or_else(|| format!("未找到描述规则 '{}'，请先使用 add-description 添加", app_name))?;
        if let Some(dn) = display_name {
            rule.display_name = Some(dn);
        }
        if let Some(desc) = description {
            rule.description = Some(desc);
        }
        self.save()
    }

    // ── 分类规则 CRUD ──

    pub fn add_category(&mut self, app_name: String, category: String) -> Result<(), String> {
        if self.app_category_rules.iter().any(|r| r.app_name == app_name) {
            return Err(format!("分类规则 '{}' 已存在，请使用 set-category 更新", app_name));
        }
        self.app_category_rules.push(AppCategoryRule { app_name, category });
        self.save()
    }

    pub fn remove_category(&mut self, app_name: &str) -> Result<(), String> {
        let before = self.app_category_rules.len();
        self.app_category_rules.retain(|r| r.app_name != app_name);
        if self.app_category_rules.len() == before {
            return Err(format!("未找到分类规则 '{}'", app_name));
        }
        self.save()
    }

    pub fn update_category(&mut self, app_name: &str, category: String) -> Result<(), String> {
        let rule = self.app_category_rules.iter_mut()
            .find(|r| r.app_name == app_name)
            .ok_or_else(|| format!("未找到分类规则 '{}'，请先使用 add-category 添加", app_name))?;
        rule.category = category;
        self.save()
    }

    // ── 列表展示 ──

    pub fn list(&self) {
        if self.app_description_rules.is_empty() && self.app_category_rules.is_empty() {
            println!("暂无规则。使用 'rsiew rule add-description' 或 'rsiew rule add-category' 添加。");
            return;
        }

        if !self.app_description_rules.is_empty() {
            println!("▸ 描述规则 ({} 条)", self.app_description_rules.len());
            println!("─────────────────────────────────────────────────────────────");
            for rule in &self.app_description_rules {
                let mut parts = vec![format!("  ● {}", rule.app_name)];
                if let Some(ref dn) = rule.display_name {
                    parts.push(format!("显示为 \"{}\"", dn));
                }
                if let Some(ref desc) = rule.description {
                    parts.push(format!("\"{}\"", desc));
                }
                println!("{}", parts.join("  "));
            }
            println!();
        }

        if !self.app_category_rules.is_empty() {
            println!("▸ 分类规则 ({} 条)", self.app_category_rules.len());
            println!("─────────────────────────────────────────────────────────────");
            for rule in &self.app_category_rules {
                println!("  ● {}  →  {}", rule.app_name, rule.category);
            }
            println!();
        }
    }
}






