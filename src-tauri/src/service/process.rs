use super::monitor::{ActiveWindow,get_active_window, tray::{TrayApplication,get_tray_applications}, taskbar::{TaskbarWindow,get_taskbar_windows}};
use super::{ocr, rule, screenshot};
use crate::config::privacy::{PrivacyAction, PrivacyFilter};
use crate::config::AppConfig;
use crate::core::error::{AppError, Result};
use std::path::Path;

/// 扩展的监控状态
#[derive(Debug, Clone)]
pub struct ExtendedMonitoringState {
    pub active_windows: Vec<ActiveWindow>,
    pub tray_applications: Vec<TrayApplication>,
    pub taskbar_windows: Vec<TaskbarWindow>,
    pub timestamp: std::time::SystemTime,
}

/// 丰富的窗口信息，包含对 AI agent 更有意义的结构化数据
#[derive(Debug, Clone)]
pub struct EnrichedWindowInfo {
    /// 应用名称（规范化后的显示名称）
    pub app_name: String,
    /// 窗口标题
    pub window_title: String,
    /// 可执行文件路径
    pub exe_path: Option<String>,
    /// 窗口边界
    pub window_bounds: Option<super::monitor::WindowBounds>,
    /// 是否最小化
    pub is_minimized: bool,
    /// 浏览器 URL（仅浏览器应用）
    pub browser_url: Option<String>,
    /// 活动分类（如 "development", "browser", "communication" 等）
    pub category: String,
    /// 截图路径（相对路径）
    pub screenshot_path: Option<String>,
    /// OCR 识别的文字内容（已过滤敏感信息）
    pub ocr_text: Option<String>,
    /// 隐私检查结果
    pub privacy_action: PrivacyAction,
    /// 人类可读的活动描述，便于 AI agent 理解用户当前在做什么
    pub description: String,
}

/// 丰富的监控状态，包含对 AI agent 更有意义的结构化数据
#[derive(Debug, Clone)]
pub struct EnrichedMonitoringState {
    /// 活动窗口信息（已丰富）
    pub active_windows: Vec<EnrichedWindowInfo>,
    /// 托盘应用程序
    pub tray_applications: Vec<TrayApplication>,
    /// 任务栏窗口
    pub taskbar_windows: Vec<TaskbarWindow>,
    /// 时间戳
    pub timestamp: std::time::SystemTime,
}

/// 原始检测函数，保持向后兼容
pub fn detect_once() -> Result<ActiveWindow> {
    if let Ok(window) = get_active_window() {
        Ok(window)
    } else {
        Err(AppError::Unknown("Failed to get active window".to_string()))
    }
}

/// 扩展的检测函数
pub fn detect_extended() -> Result<ExtendedMonitoringState> {
    let active_windows_result = get_active_window();
    let tray_apps_result = get_tray_applications();
    let taskbar_windows_result = get_taskbar_windows();
    
    let active_windows = match active_windows_result {
        Ok(window) => vec![window],
        Err(_) => Vec::new(),
    };
    
    let mut tray_applications = match tray_apps_result {
        Ok(apps) => apps,
        Err(_) => Vec::new(),
    };

    let taskbar_windows = match taskbar_windows_result {
        Ok(windows) => windows,
        Err(_) => Vec::new(),
    };

    // 排除已在任务栏窗口或活动窗口中显示的 PID，避免重复
    let excluded_pids: std::collections::HashSet<u32> = {
        let mut set = std::collections::HashSet::new();
        for win in &taskbar_windows {
            set.insert(win.pid);
        }
        set
    };

    tray_applications.retain(|app| {
        let in_taskbar = excluded_pids.contains(&app.pid);
        let in_active = active_windows.iter().any(|w| {
            w.exe_path.as_deref() == app.exe_path.as_deref()
        });
        !in_taskbar && !in_active
    });

    Ok(ExtendedMonitoringState {
        active_windows,
        tray_applications,
        taskbar_windows,
        timestamp: std::time::SystemTime::now(),
    })
}

/// 丰富单个窗口信息
/// 为 AI agent 提供更有意义的结构化信息，包括浏览器 URL、OCR 文字、活动分类等
pub fn enrich_window_info(
    window: &ActiveWindow,
    screenshot_service: &screenshot::ScreenshotService,
    data_dir: &Path,
    privacy_filter: &PrivacyFilter,
    _config: &AppConfig,
    capture_screenshot: bool,
) -> EnrichedWindowInfo {
    // 1. 解析浏览器 URL
    let browser_url = if rule::is_browser_app(&window.app_name) {
        window.browser_url.clone().or_else(|| {
            super::monitor::resolve_browser_url_for_window(
                &window.app_name,
                &window.window_title,
            )
        })
    } else {
        None
    };

    // 2. 隐私检查
    let privacy_action = privacy_filter.check_privacy_full(
        &window.app_name,
        &window.window_title,
        browser_url.as_deref(),
    );

    // 3. 活动分类
    let category = rule::categorize_app(&window.app_name, &window.window_title);

    // 4. 根据隐私策略决定是否截图和 OCR
    let (screenshot_path, ocr_text) = match privacy_action {
        PrivacyAction::Skip => (None, None),
        PrivacyAction::Anonymize => (None, None),
        PrivacyAction::Record if !capture_screenshot => (None, None),
        PrivacyAction::Record => {
            // 截图
            match screenshot_service.capture_for_window(Some(window)) {
                Ok(screenshot_result) => {
                    let relative_path = screenshot_service.get_relative_path(&screenshot_result.path);
                    
                    // OCR 识别
                    let ocr_result = {
                        let ocr_service = ocr::OcrService::new(data_dir);
                        let ocr_input_path = screenshot_result
                            .ocr_source_path
                            .as_ref()
                            .unwrap_or(&screenshot_result.path);
                        ocr_service.extract_text(ocr_input_path)
                    };

                    let ocr_text = match ocr_result {
                        Ok(Some(result)) if !result.text.is_empty() => {
                            let filtered = ocr::filter_sensitive_text(&result.text);
                            Some(filtered)
                        }
                        _ => None,
                    };

                    // 清理 OCR 临时文件（如果与归档路径不同）
                    if let Some(temp_path) = &screenshot_result.ocr_source_path {
                        if temp_path != &screenshot_result.path {
                            let _ = std::fs::remove_file(temp_path);
                        }
                    }

                    (Some(relative_path), ocr_text)
                }
                Err(e) => {
                    log::debug!("截图失败: {e}");
                    (None, None)
                }
            }
        }
    };

    // 5. 生成人类可读的活动描述
    let description = generate_activity_description(
        &window.app_name,
        &window.window_title,
        browser_url.as_deref(),
        &category,
        ocr_text.as_deref(),
    );

    EnrichedWindowInfo {
        app_name: window.app_name.clone(),
        window_title: window.window_title.clone(),
        exe_path: window.exe_path.clone(),
        window_bounds: window.window_bounds,
        is_minimized: window.is_minimized,
        browser_url,
        category,
        screenshot_path,
        ocr_text,
        privacy_action,
        description,
    }
}

/// 生成人类可读的活动描述
/// 用于帮助 AI agent 理解用户当前在做什么
fn generate_activity_description(
    app_name: &str,
    window_title: &str,
    browser_url: Option<&str>,
    category: &str,
    ocr_text: Option<&str>,
) -> String {
    let mut parts = Vec::new();

    // 应用描述
    let app_desc = match category {
        "development" => format!("正在使用开发工具 {}", app_name),
        "browser" => format!("正在使用浏览器 {}", app_name),
        "communication" => format!("正在使用通讯工具 {}", app_name),
        "office" => format!("正在使用办公软件 {}", app_name),
        "design" => format!("正在使用设计工具 {}", app_name),
        "media" => format!("正在使用媒体应用 {}", app_name),
        "system" => format!("正在使用系统工具 {}", app_name),
        _ => format!("正在使用 {}", app_name),
    };
    parts.push(app_desc);

    // 窗口标题（去除常见的无意义后缀）
    if !window_title.is_empty() && window_title != app_name {
        let clean_title = window_title
            .trim_end_matches(&format!(" - {}", app_name))
            .trim_end_matches(&format!(" — {}", app_name))
            .trim();
        if !clean_title.is_empty() && clean_title != app_name {
            parts.push(format!("窗口: {}", clean_title));
        }
    }

    // 浏览器 URL
    if let Some(url) = browser_url {
        if !url.is_empty() {
            // 提取域名作为上下文
            let domain = extract_domain_from_url(url);
            if !domain.is_empty() {
                parts.push(format!("访问网站: {}", domain));
            }
            parts.push(format!("URL: {}", url));
        }
    }

    // OCR 文字摘要（取前 200 字符）
    if let Some(text) = ocr_text {
        let summary = text.chars().take(200).collect::<String>();
        if !summary.is_empty() {
            parts.push(format!("屏幕内容: {}", summary));
        }
    }

    parts.join(" | ")
}

/// 从 URL 提取域名
fn extract_domain_from_url(url: &str) -> String {
    let url = url.trim();
    let without_protocol = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };
    
    let domain = without_protocol
        .split('/')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .split('#')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    
    domain.to_string()
}

/// 扩展的检测函数（丰富版）
/// 为 AI agent 提供更有意义的结构化信息
pub fn detect_enriched(
    screenshot_service: &screenshot::ScreenshotService,
    data_dir: &Path,
    privacy_filter: &PrivacyFilter,
    config: &AppConfig,
    capture_screenshot: bool,
) -> Result<EnrichedMonitoringState> {
    let active_windows_result = get_active_window();
    let tray_apps_result = get_tray_applications();
    let taskbar_windows_result = get_taskbar_windows();
    
    // 丰富活动窗口信息
    let active_windows = match active_windows_result {
        Ok(window) => {
            let enriched = enrich_window_info(
                &window,
                screenshot_service,
                data_dir,
                privacy_filter,
                config,
                capture_screenshot,
            );
            vec![enriched]
        }
        Err(_) => Vec::new(),
    };
    
    let mut tray_applications = match tray_apps_result {
        Ok(apps) => apps,
        Err(_) => Vec::new(),
    };

    let taskbar_windows = match taskbar_windows_result {
        Ok(windows) => windows,
        Err(_) => Vec::new(),
    };

    // 排除已在任务栏窗口或活动窗口中显示的 PID，避免重复
    let excluded_pids: std::collections::HashSet<u32> = {
        let mut set = std::collections::HashSet::new();
        for win in &taskbar_windows {
            set.insert(win.pid);
        }
        set
    };

    tray_applications.retain(|app| {
        let in_taskbar = excluded_pids.contains(&app.pid);
        let in_active = active_windows.iter().any(|w| {
            w.exe_path.as_deref() == app.exe_path.as_deref()
        });
        !in_taskbar && !in_active
    });

    Ok(EnrichedMonitoringState {
        active_windows,
        tray_applications,
        taskbar_windows,
        timestamp: std::time::SystemTime::now(),
    })
}
