use crate::config::rule::RuleConfig;
use crate::service::process;

/// 从浏览器窗口标题推断标签页数量
/// Edge/Chrome 的窗口标题格式：
/// - "页面标题 - 个人 - Microsoft Edge" (1个标签页)
/// - "页面标题 和另外 N 个页面 - 个人 - Microsoft Edge" (N+1个标签页)
fn extract_tab_count_from_title(window_title: &str) -> usize {
    // 检查是否包含 "和另外 N 个页面"
    if let Some(pos) = window_title.find("和另外") {
        let after = &window_title[pos + "和另外".len()..];
        if let Some(end_pos) = after.find("个页面") {
            let num_str = &after[..end_pos].trim();
            if let Ok(n) = num_str.parse::<usize>() {
                return n + 1; // +1 因为当前页面也算一个
            }
        }
    }
    
    // 检查英文格式 "and N more pages"
    if let Some(pos) = window_title.find("and") {
        let after = &window_title[pos + "and".len()..];
        if let Some(end_pos) = after.find("more") {
            let num_str = &after[..end_pos].trim();
            if let Ok(n) = num_str.parse::<usize>() {
                return n + 1;
            }
        }
    }
    
    // 默认返回1（只有一个标签页）
    1
}




pub fn watch_commands(all: bool, one: bool, tray: bool, taskbar: bool, ignore: Option<String>) {
    use crate::utils::activity_classifier::classify_activity;
    use crate::service::rule::resolve::find_app_description;
    use chrono::{DateTime, Local};

    let ignored = ignore.map(|s| s.to_lowercase());
    let is_ignored = |name: &str| -> bool {
        if let Some(ref ign) = ignored {
            name.to_lowercase().contains(ign)
        } else {
            false
        }
    };

    // 加载规则配置
    let rules = RuleConfig::load();

    fn fmt_timestamp(st: &std::time::SystemTime) -> String {
        let dt: DateTime<Local> = (*st).into();
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    fn window_state_label(is_minimized: bool) -> &'static str {
        if is_minimized { "最小化" } else { "正常" }
    }

    fn fmt_bounds(bounds: Option<&crate::service::monitor::WindowBounds>) -> String {
        match bounds {
            Some(b) => format!("({}, {}) {}x{}", b.x, b.y, b.width, b.height),
            None => "未知".to_string(),
        }
    }

    fn print_classification(app_name: &str, title: &str, url: Option<&str>) {
        let cls = classify_activity(app_name, title, url);
        if cls.confidence >= 55 {
            println!("    分类: {} ({}%)", cls.semantic_category, cls.confidence);
        } else {
            println!("    分类: {}", cls.semantic_category);
        }
    }

    if all {
        match process::detect_extended() {
            Ok(state) => {
                println!("┌─────────────────────────────────────────────────────────────┐");
                println!("│  系统活动监控  │  {}  │", fmt_timestamp(&state.timestamp));
                println!("└─────────────────────────────────────────────────────────────┘");
                println!();

                // 活动窗口
                let active_windows: Vec<_> = state.active_windows.iter()
                    .filter(|w| !is_ignored(&w.app_name))
                    .collect();
                if !active_windows.is_empty() {
                    println!("▸ 当前活动窗口");
                    println!("─────────────────────────────────────────────────────────────");
                    for win in &active_windows {
                        println!("  ● {}", win.app_name);
                        if !win.window_title.is_empty() {
                            let clean_title = win.window_title
                                .trim_end_matches(&format!(" - {}", win.app_name))
                                .trim_end_matches(&format!(" — {}", win.app_name))
                                .trim();
                            if !clean_title.is_empty() && clean_title != win.app_name {
                                println!("    {}", clean_title);
                            }
                        }
                        if let Some(ref url) = win.browser_url {
                            if !url.is_empty() {
                                println!("    URL: {}", url);
                            }
                        }
                        println!("    状态: {} {}", 
                            window_state_label(win.is_minimized),
                            fmt_bounds(win.window_bounds.as_ref()));
                        print_classification(
                            &win.app_name,
                            &win.window_title,
                            win.browser_url.as_deref(),
                        );
                        println!();
                    }
                }

                // 任务栏窗口
                let taskbar_windows: Vec<_> = state.taskbar_windows.iter()
                    .filter(|w| !is_ignored(&w.app_name))
                    .collect();
                if !taskbar_windows.is_empty() {
                    println!("▸ 任务栏窗口 ({} 个)", taskbar_windows.len());
                    println!("─────────────────────────────────────────────────────────────");
                    for win in &taskbar_windows {
                        let mut info = format!("  ● {}", win.app_name);
                        if win.is_minimized {
                            info.push_str(" [最小化]");
                        }
                        println!("{}", info);
                        
                        if !win.window_title.is_empty() {
                            let clean_title = win.window_title
                                .trim_end_matches(&format!(" - {}", win.app_name))
                                .trim_end_matches(&format!(" — {}", win.app_name))
                                .trim();
                            if !clean_title.is_empty() && clean_title != win.app_name {
                                println!("    {}", clean_title);
                            }
                        }
                        
                        // 对于浏览器窗口，尝试获取标签页信息
                        if crate::service::rule::is_browser_app(&win.app_name) {
                            let browser_url = crate::service::monitor::resolve_browser_url_for_hwnd(
                                &win.app_name,
                                &win.window_title,
                                win.hwnd,
                            );
                            if let Some(ref url) = browser_url {
                                if !url.is_empty() {
                                    println!("    当前标签页 URL: {}", url);
                                }
                            }
                            
                            // 从窗口标题推断标签页数量
                            // Edge/Chrome 的窗口标题格式通常是 "页面标题 - 个人 - Microsoft Edge"
                            // 或 "页面标题 和另外 N 个页面 - 个人 - Microsoft Edge"
                            let tab_count = extract_tab_count_from_title(&win.window_title);
                            if tab_count > 1 {
                                println!("    标签页: {} 个 ", tab_count);
                            }
                            
                            // 尝试获取所有标签页（如果 UI Automation 可用）
                            let browser_tabs = crate::service::monitor::browser::get_browser_tabs(
                                win.hwnd,
                                &win.app_name,
                            );
                            if let Some(browser_info) = browser_tabs {
                                if !browser_info.tabs.is_empty() {
                                    println!("    检测到的标签页:");
                                    for (i, tab) in browser_info.tabs.iter().enumerate() {
                                        let marker = if tab.is_active { "►" } else { " " };
                                        let title = if tab.title.len() > 50 {
                                            format!("{}...", &tab.title[..47])
                                        } else {
                                            tab.title.clone()
                                        };
                                        println!("      {} {}", marker, title);
                                        if i >= 9 {
                                            println!("      ... 还有 {} 个标签页", browser_info.tabs.len() - 10);
                                            break;
                                        }
                                    }
                                }
                            }
                        } else {
                            let browser_url = crate::service::monitor::resolve_browser_url_for_hwnd(
                                &win.app_name,
                                &win.window_title,
                                win.hwnd,
                            );
                            if let Some(ref url) = browser_url {
                                if !url.is_empty() {
                                    println!("    URL: {}", url);
                                }
                            }
                        }
                        print_classification(&win.app_name, &win.window_title, None);
                        println!();
                    }
                }

                // 托盘程序
                let tray_apps: Vec<_> = state.tray_applications.iter()
                    .filter(|a| !is_ignored(&a.app_name))
                    .collect();
                if !tray_apps.is_empty() {
                    println!("▸ 托盘程序 ({} 个)", tray_apps.len());
                    println!("─────────────────────────────────────────────────────────────");

                    for app in &tray_apps {
                        let desc = find_app_description(&rules.app_description_rules, &app.app_name);
                        let display_name = desc.as_ref()
                            .and_then(|d| d.display_name.as_deref())
                            .unwrap_or(&app.app_name);
                        let description = desc.as_ref()
                            .and_then(|d| d.description.as_deref());

                        let visibility = if app.is_visible { "可见" } else { "隐藏" };
                        println!("  ● {} [{}]", display_name, visibility);
                        if let Some(desc_text) = description {
                            println!("    {}", desc_text);
                        }
                        if let Some(ref path) = app.exe_path {
                            println!("    {}", path);
                        }
                    }
                    println!();
                }
            }
            Err(e) => println!("错误: {}", e),
        }
    } else if one {
        let window = match process::detect_once() {
            Ok(window) => window,
            Err(e) => {
                println!("错误: {}", e);
                return;
            }
        };
        if is_ignored(&window.app_name) {
            println!("当前活动窗口已被忽略: {}", window.app_name);
            return;
        }
        println!("当前活动窗口: {}", window.app_name);
        if !window.window_title.is_empty() {
            let clean_title = window.window_title
                .trim_end_matches(&format!(" - {}", window.app_name))
                .trim_end_matches(&format!(" — {}", window.app_name))
                .trim();
            if !clean_title.is_empty() && clean_title != window.app_name {
                println!("  {}", clean_title);
            }
        }
        if let Some(ref url) = window.browser_url {
            if !url.is_empty() {
                println!("  URL: {}", url);
            }
        }
        print_classification(&window.app_name, &window.window_title, window.browser_url.as_deref());

    } else if tray {
        println!("Watching tray applications in real-time");
    } else if taskbar {
        println!("Watching taskbar windows in real-time");
    } else {
        println!("No watch option selected");
    }
}
