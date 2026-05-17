use serde::{Deserialize, Serialize};

/// 浏览器标签页信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserTab {
    /// 标签页标题
    pub title: String,
    /// 标签页 URL（如果能获取到）
    pub url: Option<String>,
    /// 是否为当前活动标签页
    pub is_active: bool,
}

/// 浏览器窗口信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserWindowInfo {
    /// 浏览器名称
    pub browser_name: String,
    /// 窗口标题
    pub window_title: String,
    /// 标签页列表
    pub tabs: Vec<BrowserTab>,
}

/// 通过 UI Automation 获取浏览器窗口的所有标签页
/// 使用 Edge/Chrome 的标签页类名 "EdgeTab" 进行搜索
#[cfg(target_os = "windows")]
pub fn get_browser_tabs(hwnd: isize, browser_name: &str) -> Option<BrowserWindowInfo> {
    use uiautomation::types::{ Handle};
    use uiautomation::UIAutomation;

    if hwnd == 0 {
        return None;
    }

    log::debug!("get_browser_tabs: 开始获取 {} 的标签页", browser_name);

    let automation = UIAutomation::new().ok()?;
    let window_element = automation.element_from_handle(Handle::from(hwnd)).ok()?;

    let mut tabs = Vec::new();
    let mut window_title = String::new();

    // 获取窗口标题
    if let Ok(name) = window_element.get_name() {
        window_title = name;
    }

    // Edge 标签页的类名
    let edge_tab_class = if browser_name.to_lowercase().contains("edge") {
        "EdgeTab"
    } else if browser_name.to_lowercase().contains("chrome") {
        "ChromeTab"
    } else {
        "EdgeTab"
    };

    log::debug!("get_browser_tabs: 搜索类名为 {} 的元素", edge_tab_class);

    // 使用 Matcher 在窗口内搜索特定类名的元素
    let matcher = automation.create_matcher()
        .from(window_element.clone())
        .classname(edge_tab_class)
        .timeout(500);

    if let Ok(elements) = matcher.find_all() {
        log::debug!("get_browser_tabs: 找到 {} 个 {} 元素", elements.len(), edge_tab_class);
        
        for elem in elements {
            let title = elem.get_name().unwrap_or_default();
            if !title.is_empty() {
                log::debug!("get_browser_tabs: 标签页 '{}'", title);
                tabs.push(BrowserTab {
                    title,
                    url: None,
                    is_active: false,
                });
            }
        }
    } else {
        log::debug!("get_browser_tabs: 未找到 {} 元素", edge_tab_class);
    }

    // 如果找到了标签页，尝试确定哪个是活动的
    if !tabs.is_empty() {
        // 通过窗口标题来确定活动标签页
        let active_title = extract_active_tab_title(&window_title, browser_name);
        
        for tab in &mut tabs {
            if tab.title == active_title {
                tab.is_active = true;
                break;
            }
        }
        
        // 如果没有匹配的，假设第一个是活动的
        if !tabs.iter().any(|t| t.is_active) {
            if let Some(first) = tabs.first_mut() {
                first.is_active = true;
            }
        }
    }

    log::debug!("get_browser_tabs: 返回 {} 个标签页", tabs.len());
    Some(BrowserWindowInfo {
        browser_name: browser_name.to_string(),
        window_title,
        tabs,
    })
}

/// 从窗口标题中提取当前活动标签页的标题
#[cfg(target_os = "windows")]
fn extract_active_tab_title(window_title: &str, browser_name: &str) -> String {
    let suffix_patterns = if browser_name.to_lowercase().contains("edge") {
        vec![" - 个人 - Microsoft Edge", " - Microsoft Edge"]
    } else if browser_name.to_lowercase().contains("chrome") {
        vec![" - Google Chrome"]
    } else {
        vec![]
    };
    
    let mut title = window_title.to_string();
    
    for pattern in &suffix_patterns {
        if let Some(pos) = title.find(pattern) {
            title = title[..pos].to_string();
            break;
        }
    }
    
    // 移除 "和另外 N 个页面" 部分
    if let Some(pos) = title.find(" 和另外") {
        title = title[..pos].to_string();
    }
    
    title.trim().to_string()
}

/// 非 Windows 平台的占位实现
#[cfg(not(target_os = "windows"))]
pub fn get_browser_tabs(_hwnd: isize, _browser_name: &str) -> Option<BrowserWindowInfo> {
    None
}
