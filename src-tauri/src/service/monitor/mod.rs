pub mod tray;
pub mod browser;
pub mod taskbar;

use super::rule;
use crate::core::error::{AppError, Result};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use std::process::Output;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use std::thread;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use std::time::{Duration, Instant};
#[cfg(target_os = "windows")]
use winapi::shared::windef::RECT;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
const MONITOR_COMMAND_TIMEOUT: Duration = Duration::from_millis(1200);

/// 活动窗口信息
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct ActiveWindow {
    pub is_minimized: bool,
    pub app_name: String,
    pub window_title: String,
    pub exe_path: Option<String>,
    pub browser_url: Option<String>,
    pub window_bounds: Option<WindowBounds>,
}


#[cfg(target_os = "windows")]
fn normalize_executable_path(path: &str) -> Option<String> {
    let trimmed = path.trim().trim_matches('"');
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.replace('/', "\\"))
}

/// 通过 QueryFullProcessImageNameW 获取进程可执行文件名，仅需低权限
/// 返回 exe 文件名（不含路径，如 "WINWORD.EXE"），作为 GetModuleBaseNameW 的备用
#[cfg(target_os = "windows")]
fn get_process_name_by_image(pid: u32) -> Option<String> {
    get_process_image_path(pid).and_then(|full_path| {
        full_path
            .split('\\')
            .last()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    })
}

#[cfg(target_os = "windows")]
fn get_process_image_path(pid: u32) -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winbase::QueryFullProcessImageNameW;

    unsafe {
        // 只需 PROCESS_QUERY_LIMITED_INFORMATION，对 UAC 保护进程也有效
        let handle = OpenProcess(0x1000, 0, pid);
        if handle.is_null() {
            return None;
        }

        let mut buf: [u16; 512] = [0; 512];
        let mut size: u32 = 512;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);

        if ok == 0 || size == 0 {
            return None;
        }

        normalize_executable_path(
            &OsString::from_wide(&buf[..size as usize])
                .to_string_lossy()
                .to_string(),
        )
    }
}

// ==========================================
// Windows 实现 (WinAPI)
// ==========================================
#[cfg(target_os = "windows")]
pub fn get_active_window() -> Result<ActiveWindow>{
    get_active_window_with_option(true)
}


#[cfg(target_os = "windows")]
pub fn get_active_window_with_option(include_browser_url: bool) -> Result<ActiveWindow> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::psapi::GetModuleBaseNameW;
    use winapi::um::winnt::PROCESS_QUERY_INFORMATION;
    use winapi::um::winuser::{
        GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    };

    const PROCESS_QUERY_LIMITED: u32 = 0x1000;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            // null HWND 出现在睡眠/现代待机唤醒、UAC弹窗、窗口切换瞬间等场景
            // 此时没有真实的前台窗口，不应伪造应用名，由调用方决定如何处理
            return Err(AppError::Unknown("没有前台窗口".to_string()));
        }
        // 获取窗口标题
        let mut title: [u16; 512] = [0; 512];
        let len = GetWindowTextW(hwnd, title.as_mut_ptr(), 512);
        let window_title = if len > 0 {
            OsString::from_wide(&title[..len as usize])
                .to_string_lossy()
                .to_string()
        } else {
            String::new()
        };

        // 获取进程ID
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);

        let exe_path = if pid > 0 {
            get_process_image_path(pid)
        } else {
            None
        };
        let raw_app_name = if pid > 0 {
            // 方法一：PROCESS_QUERY_LIMITED_INFORMATION + GetModuleBaseNameW
            // 对大多数普通进程（Word、VSCode、WPS 等）有效
            let handle = OpenProcess(PROCESS_QUERY_LIMITED, 0, pid);
            let name_opt = if !handle.is_null() {
                let mut name: [u16; 256] = [0; 256];
                let len = GetModuleBaseNameW(handle, std::ptr::null_mut(), name.as_mut_ptr(), 256);
                CloseHandle(handle);
                if len > 0 {
                    Some(
                        OsString::from_wide(&name[..len as usize])
                            .to_string_lossy()
                            .to_string(),
                    )
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(n) = name_opt {
                n
            } else {
                // 方法二：回退完整权限（覆盖 GetModuleBaseNameW 需要 PROCESS_VM_READ 的场景）
                let handle2 = OpenProcess(PROCESS_QUERY_INFORMATION | 0x0010, 0, pid);
                let name_opt2 = if !handle2.is_null() {
                    let mut name: [u16; 256] = [0; 256];
                    let len =
                        GetModuleBaseNameW(handle2, std::ptr::null_mut(), name.as_mut_ptr(), 256);
                    CloseHandle(handle2);
                    if len > 0 {
                        Some(
                            OsString::from_wide(&name[..len as usize])
                                .to_string_lossy()
                                .to_string(),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(n) = name_opt2 {
                    n
                } else {
                    // 方法三：QueryFullProcessImageNameW，只需低权限，返回完整路径取文件名
                    get_process_name_by_image(pid).unwrap_or_else(|| {
                        // 方法四：从窗口标题最后一段推断（如 "文件名 - 应用名" 取最后段）
                        // 避免进程全部落入 Unknown 导致时长无法区分统计
                        if let Some(name_from_path) = exe_path.as_deref().and_then(|path| {
                            std::path::Path::new(path)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .map(|name| name.to_string())
                        }) {
                            name_from_path
                        } else if !window_title.is_empty() {
                            window_title
                                .split(" - ")
                                .last()
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty() && s.len() < 40)
                                .unwrap_or_else(|| "Unknown".to_string())
                        } else {
                            "Unknown".to_string()
                        }
                    })
                }
            }
        } else {
            "Unknown".to_string()
        };
        let app_name = rule::normalize_display_app_name(&raw_app_name);
        let is_minimized = IsIconic(hwnd) != 0;
        let browser_url = if include_browser_url {
            get_browser_url_windows(&raw_app_name, &window_title, hwnd as isize)
        } else {
            None
        };
        let window_bounds = {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if GetWindowRect(hwnd, &mut rect) != 0 {
                let width = (rect.right - rect.left).max(0) as u32;
                let height = (rect.bottom - rect.top).max(0) as u32;
                if width > 0 && height > 0 {
                    Some(WindowBounds {
                        x: rect.left,
                        y: rect.top,
                        width,
                        height,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        };
                Ok(ActiveWindow {
            app_name,
            window_title,
            browser_url,
            exe_path,
            window_bounds,
            is_minimized,
        })
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_overlay_windows(_frontmost_app: &str) -> Vec<ActiveWindow> {
    Vec::new()
}

/// 获取浮动/overlay 窗口（如 PiP 画中画小窗）
/// 通过 CGWindowListCopyWindowInfo 枚举屏幕上所有窗口，
/// 过滤出 layer > 0 的浮动窗口（排除当前前台应用和系统进程）
#[cfg(target_os = "macos")]
pub fn get_overlay_windows(frontmost_app: &str) -> Vec<ActiveWindow> {
    use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
    use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumberRef;
    use core_foundation::string::CFString;
    use core_graphics::display::{
        kCGNullWindowID, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
        CGWindowListCopyWindowInfo,
    };

    // 系统进程排除列表（覆盖英文和中文 macOS 系统下的进程名）
    const SYSTEM_PROCESSES: &[&str] = &[
        "Window Server",
        "Dock",
        "程序坞",
        "SystemUIServer",
        "Control Center",
        "控制中心",
        "Spotlight",
        "聚焦",
        "NotificationCenter",
        "通知中心",
        "Finder",
        "访达",
        "TextInputMenuAgent",
        "Wallpaper",
        "WindowManager",
        "AirPlayUIAgent",
        "Siri",
        "loginwindow",
        "ControlStrip",
        "CoreServicesUIAgent",
        "ScreenSaverEngine",
        "universalAccessAuthWarn",
    ];

    // 已知会产生无用浮动工具栏/面板的应用
    // 这些应用的浮动窗口（非前台时）几乎一定是悬浮工具栏，不应计为独立使用时长
    const TOOLBAR_APPS: &[&str] = &[
        "WPS Office",
        "wpsoffice",
        "WPS",
        "Microsoft Word",
        "Microsoft Excel",
        "Microsoft PowerPoint",
    ];

    let mut results: Vec<ActiveWindow> = Vec::new();

    unsafe {
        let window_list = CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        );
        if window_list.is_null() {
            return results;
        }

        let count = CFArrayGetCount(window_list as _);

        for i in 0..count {
            let dict = CFArrayGetValueAtIndex(window_list as _, i) as CFDictionaryRef;
            if dict.is_null() {
                continue;
            }

            // 读取 kCGWindowLayer
            let layer_key = CFString::new("kCGWindowLayer");
            let mut layer_ref: CFTypeRef = std::ptr::null();
            if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                dict,
                layer_key.as_CFTypeRef() as *const _,
                &mut layer_ref,
            ) == 0
                || layer_ref.is_null()
            {
                continue;
            }
            let mut layer: i32 = 0;
            if !core_foundation::number::CFNumberGetValue(
                layer_ref as CFNumberRef,
                core_foundation::number::kCFNumberSInt32Type,
                &mut layer as *mut i32 as *mut _,
            ) {
                continue;
            }

            // 只取浮动窗口 (layer > 0)
            if layer <= 0 {
                continue;
            }

            // 读取 kCGWindowOwnerName
            let owner_key = CFString::new("kCGWindowOwnerName");
            let mut owner_ref: CFTypeRef = std::ptr::null();
            if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                dict,
                owner_key.as_CFTypeRef() as *const _,
                &mut owner_ref,
            ) == 0
                || owner_ref.is_null()
            {
                continue;
            }
            let owner_cfstr =
                core_foundation::string::CFString::wrap_under_get_rule(owner_ref as _);
            let owner_name = owner_cfstr.to_string();

            // 排除当前前台应用（避免重复计时）
            if owner_name == frontmost_app {
                continue;
            }

            // 排除系统进程（使用包含匹配，兼容中英文系统名称差异）
            if SYSTEM_PROCESSES
                .iter()
                .any(|&sys| owner_name == sys || owner_name.contains(sys))
            {
                continue;
            }

            // 排除已知悬浮工具栏应用的浮动窗口
            if TOOLBAR_APPS.iter().any(|&app| owner_name.contains(app)) {
                log::debug!("🪟 排除工具栏浮动窗口: {} (layer={})", owner_name, layer);
                continue;
            }

            // 读取窗口尺寸 kCGWindowBounds
            let bounds_key = CFString::new("kCGWindowBounds");
            let mut bounds_ref: CFTypeRef = std::ptr::null();
            if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                dict,
                bounds_key.as_CFTypeRef() as *const _,
                &mut bounds_ref,
            ) == 0
                || bounds_ref.is_null()
            {
                continue;
            }
            // kCGWindowBounds 是一个 CFDictionary: {Height, Width, X, Y}
            let bounds_dict = bounds_ref as CFDictionaryRef;

            let width = get_cf_dict_number(bounds_dict, "Width").unwrap_or(0.0);
            let height = get_cf_dict_number(bounds_dict, "Height").unwrap_or(0.0);

            // 排除小图标/指示器/工具栏类窗口
            // WPS Office 等应用常驻的悬浮工具栏尺寸较小，需要提高阈值
            if width <= 200.0 || height <= 150.0 {
                continue;
            }

            // 读取 kCGWindowName（可选）
            let win_name_key = CFString::new("kCGWindowName");
            let mut win_name_ref: CFTypeRef = std::ptr::null();
            let window_title = if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                dict,
                win_name_key.as_CFTypeRef() as *const _,
                &mut win_name_ref,
            ) != 0
                && !win_name_ref.is_null()
            {
                let name_cfstr =
                    core_foundation::string::CFString::wrap_under_get_rule(win_name_ref as _);
                name_cfstr.to_string()
            } else {
                String::new()
            };

            // 无窗口标题的浮动窗口大概率是工具栏/面板/悬浮球，用更严格的阈值
            if window_title.is_empty() && (width <= 400.0 || height <= 300.0) {
                continue;
            }

            log::debug!(
                "🪟 检测到浮动窗口: {} - {} (layer={}, {}x{})",
                owner_name,
                window_title,
                layer,
                width as i32,
                height as i32
            );

            results.push(ActiveWindow {
                app_name: owner_name,
                window_title,
                browser_url: None,
                executable_path: None,
                window_bounds: None,
                is_minimized: false,
            });
        }

        CFRelease(window_list as _);
    }

    // 去重：同一应用可能有多个浮动窗口，只保留第一个
    results.dedup_by(|a, b| a.app_name == b.app_name);

    results
}

#[cfg(target_os = "linux")]
pub fn get_active_window() -> Result<ActiveWindow, String> {
    let xprop_id = Command::new("xprop")
        .args(["-root", "-_NET_ACTIVE_WINDOW"])
        .output()
        .map_err(|e| format!("Failed to execute xprop: {}", e))?;

    if !xprop_id.status.success() {
        return Err(format!(
            "xprop failed: {}",
            String::from_utf8_lossy(&xprop_id.stderr)
        ));
    }
    let id_out = String::from_utf8_lossy(&xprop_id.stdout);

    let wind_id = id_out
        .split_whitespace()
        .last()
        .ok_or_else(|| "Failed to parse active window ID".to_string())?;

    if wind_id == "0x0" {
        return Err("No active window".to_string());
    }

    let xprop_props = Command::new("xprop")
        .args(["-id", wind_id, "WM_CLASS", "_NET_WM_NAME", "WM_NAME"])
        .output()
        .map_err(|e| format!("Failed to run xprop for window {}: {}", wind_id, e))?;

    let props_out = String::from_utf8_lossy(&xprop_props.stdout);

    let mut app_name = String::new();
    let mut window_title = String::new();

    for line in props_out.lines() {
        if line.starts_with("WM_CLASS") {
            if let Some(class_str) = line.split('=').nth(1) {
                let parts: Vec<&str> = class_str.split(',').collect();
                let target = parts.last().unwrap_or(&parts[0]).trim();
                app_name = target.trim_matches('"').to_string()
            }
        } else if line.starts_with("_NET_WM_NAME")
            || (window_title.is_empty() && line.starts_with("WM_NAME"))
        {
            if let Some(title_str) = line.split('=').nth(1) {
                window_title = title_str.trim().trim_matches('"').to_string()
            }
        }
    }
    if app_name.is_empty() {
        app_name = "Unknown".to_string();
    }

    let mut exe_path = String::new();
    if let Ok(pid_output) = Command::new("xprop")
        .args(["-id", wind_id, "_NET_WM_PID"])
        .output()
    {
        let pid_str = String::from_utf8_lossy(&pid_output.stdout);
        if let Some(pid) = pid_str.split('=').nth(1).map(|s| s.trim()) {
            if let Ok(path) = std::fs::read_link(format!("/proc/{}/exe", pid)) {
                exe_path = path.to_string_lossy().to_string();
            }
        }
    }

    Ok(ActiveWindow {
        app_name: normalize_display_app_name(&app_name, &window_title),
        window_title,
        exe_path,
    })
}

pub fn normalize_display_app_name(raw_name: &str, window_title: &str) -> String {
    let lower_name = raw_name.to_lowercase();
    let lower_title = window_title.to_lowercase();
    if lower_name.contains("electron") || lower_name.contains("helper") {
        if lower_title.contains("visual studio code") || lower_title.contains("vs code") {
            return "VS Code".to_string();
        }
        if lower_title.contains("discord") {
            return "Discord".to_string();
        }
        if lower_title.contains("figma") {
            return "Figma".to_string();
        }
    }
    match lower_name.as_str() {
        "msedge" => "Microsoft Edge".to_string(),
        "chrome" => "Google Chrome".to_string(),
        "firefox" => "Google Chrome".to_string(),
        "code" | "code - insiders" => "VS Code".to_string(),
        "idea64" | "idea" => "IntelliJ IDEA".to_string(),
        "pycharm64" | "pycharm" => "PyCharm".to_string(),
        "goland64" | "goland" => "GoLand".to_string(),
        "wechat" => "WeChat".to_string(),
        "qq" => "Tencent QQ".to_string(),
        "explorer" => "File Explorer".to_string(),
        "cmd" | "powershell" | "wt" => "Terminal".to_string(),
        "devenv" => "Visual Studio".to_string(),
        "dingtalk" => "DingTalk".to_string(),
        "cursor" => "Cursor".to_string(),
        "postman" => "Postman".to_string(),
        _ => {
            let mut c = raw_name.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub fn get_active_window() -> Result<ActiveWindow, String> {
    Ok(ActiveWindow {
        app_name: normalize_display_app_name("MockApp", "MockTitle"),
        window_title: "MockTitle".to_string(),
        exe_path: "/mock/path/MockApp.app".to_string(),
    })
}

#[cfg(target_os = "macos")]
pub fn resolve_browser_url_for_window(app_name: &str, window_title: &str) -> Option<String> {
    get_browser_url(app_name, window_title)
}

#[cfg(target_os = "linux")]
pub fn resolve_browser_url_for_window(app_name: &str, window_title: &str) -> Option<String> {
    resolve_browser_url_for_window_linux(app_name, window_title)
}

#[cfg(target_os = "macos")]
fn get_browser_url_via_system_events(process_name: &str) -> Option<String> {
    let script = browser_url_ui_script_macos(process_name);
    let output = run_monitor_command_with_timeout(
        Command::new("osascript").arg("-e").arg(script),
        &format!("{process_name} URL UI 采集"),
    )
    .ok()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!("获取 {process_name} UI URL 失败: {}", stderr.trim());
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if process_name == "Zen" {
        let preview = browser_url_candidates_preview_from_output(&stdout, 8);
        if !preview.is_empty() {
            log::info!("Zen UI URL 候选: {}", preview.join(" | "));
        }
    }
    let url = best_browser_url_candidate_from_output(&stdout);
    if let Some(ref url) = url {
        log_browser_url_once(
            &format!("ui:{process_name}"),
            &format!("获取到 {process_name} UI URL"),
            url,
        );
    }
    url
}

/// 通过原生 UI Automation COM 接口获取浏览器地址栏 URL
/// 使用 HWND 精准定位浏览器窗口，查找 Edit 控件并读取 ValuePattern
#[cfg(target_os = "windows")]
pub fn get_url_via_uiautomation(hwnd: isize) -> Option<String> {
    use uiautomation::patterns::{UILegacyIAccessiblePattern, UIValuePattern};
    use uiautomation::types::{ControlType, Handle};
    use uiautomation::UIAutomation;

    let automation = UIAutomation::new().ok()?;
    // Handle 内部字段在 0.24.4 变为私有，改用 From trait 构造
    let window_element = automation.element_from_handle(Handle::from(hwnd)).ok()?;

    let mut best_match: Option<(i32, String)> = None;

    let inspect_control = |control: uiautomation::UIElement,
                           best_match: &mut Option<(i32, String)>| {
        let control_type = match control.get_control_type() {
            Ok(t) => t,
            Err(_) => return,
        };

        if control_type != ControlType::Edit && control_type != ControlType::Document {
            return;
        }

        let name = control.get_name().unwrap_or_default();
        let class_name = control.get_classname().unwrap_or_default();
        let name_lower = name.to_lowercase();
        let class_lower = class_name.to_lowercase();
        let address_like = name_lower.contains("address")
            || name_lower.contains("地址")
            || name_lower.contains("location")
            || name_lower.contains("omnibox")
            || class_lower.contains("omnibox")
            || class_lower.contains("address");

        let mut candidates = Vec::new();
        if let Ok(pattern) = control.get_pattern::<UIValuePattern>() {
            if let Ok(value) = pattern.get_value() {
                candidates.push(value);
            }
        }
        if let Ok(pattern) = control.get_pattern::<UILegacyIAccessiblePattern>() {
            if let Ok(value) = pattern.get_value() {
                candidates.push(value);
            }
        }
        candidates.push(name.clone());

        for raw in candidates {
            let Some(url) = rule::normalize_possible_url(&raw) else {
                continue;
            };

            let mut score = match control_type {
                ControlType::Edit => 35,
                ControlType::Document => 15,
                _ => 0,
            };

            if address_like {
                score += 50;
            }
            if raw.starts_with("http://") || raw.starts_with("https://") {
                score += 30;
            } else if raw == class_name || raw == name {
                score += 5;
            }

            if score >= 60
                && best_match
                    .as_ref()
                    .map(|(best_score, _)| score > *best_score)
                    .unwrap_or(true)
            {
                *best_match = Some((score, url));
            }
        }
    };

    // 先扫描全部 Edit 控件。
    // Chrome/Chromium 的地址栏在不同版本和 UI 状态下不一定是第一个 Edit；
    // 只取 find_first 很容易误拿到页面内搜索框，导致 URL 统计长期为空。
    if let Ok(edits) = automation
        .create_matcher()
        .from(window_element.clone())
        .control_type(ControlType::Edit)
        .timeout(300)
        .find_all()
    {
        for edit in edits {
            inspect_control(edit, &mut best_match);
        }
    }
    if let Some((score, url)) = &best_match {
        if *score >= 85 {
            return Some(url.clone());
        }
    }

    // 再扫 Document 控件作为补充。
    // 某些浏览器或特殊页面会把可读 URL 暴露在 Document，而不是地址栏 Edit。
    if let Ok(docs) = automation
        .create_matcher()
        .from(window_element)
        .control_type(ControlType::Document)
        .timeout(300)
        .find_all()
    {
        for doc in docs {
            inspect_control(doc, &mut best_match);
        }
    }

    best_match.map(|(_, url)| url)
}

/// 从窗口获取浏览器 URL (Windows)
/// 使用原生 UI Automation COM 接口（通过 uiautomation crate），不再 spawn PowerShell 进程
/// 为避免串号，不缓存正向结果，优先保证 URL 与时长归属的准确性
#[cfg(target_os = "windows")]
fn get_browser_url_windows(app_name: &str, window_title: &str, hwnd: isize) -> Option<String> {
    if !rule::is_browser_app(app_name) {
        return None;
    }

    // 使用原生 UI Automation 获取 URL，catch_unwind 防止 COM 异常导致崩溃
    let native_result = std::panic::catch_unwind(|| get_url_via_uiautomation(hwnd)).unwrap_or(None);
    if let Some(url) = native_result {
        log::debug!("浏览器 URL 命中原生 UIA: {url}");
        return Some(url);
    }

    let powershell_result = get_url_via_powershell_uia(hwnd);
    if let Some(url) = powershell_result {
        log::debug!("浏览器 URL 命中 PowerShell UIA: {url}");
        return Some(url);
    }

    // UI Automation 失败时，尝试从窗口标题提取域名信息作为兜底
    let title_result = rule::infer_browser_page_hint(window_title);
    if title_result.is_none() {
        log::debug!(
            "浏览器 URL 获取失败: app={}, title={}",
            app_name,
            window_title
        );
    }
    title_result
}

#[cfg(any(target_os = "linux", test))]
fn get_browser_url_linux(app_name: &str, window_title: &str) -> Option<String> {
    if !rule::is_browser_app(app_name) {
        return None;
    }

    let app_lower = app_name.to_lowercase();

    if rule::matches_firefox_family_browser(&app_lower) {
        if let Some(url) = firefox_family_session_store_url(app_name, window_title) {
            return Some(url);
        }
    }

    rule::extract::extract_url_from_title(window_title)
}

#[cfg(any(target_os = "linux", test))]
fn resolve_browser_url_for_window_linux(app_name: &str, window_title: &str) -> Option<String> {
    get_browser_url_linux(app_name, window_title)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn resolve_browser_url_for_window(app_name: &str, window_title: &str) -> Option<String> {
    if !rule::is_browser_app(app_name) {
        return None;
    }
    // 获取前台窗口 hwnd 并尝试读取浏览器 URL
    let hwnd = unsafe { winapi::um::winuser::GetForegroundWindow() };
    if hwnd.is_null() {
        return None;
    }
    get_browser_url_windows(app_name, window_title, hwnd as isize)
}

/// 从指定 HWND 的浏览器窗口获取 URL（用于任务栏窗口等非前台窗口）
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn resolve_browser_url_for_hwnd(app_name: &str, window_title: &str, hwnd: isize) -> Option<String> {
    if !rule::is_browser_app(app_name) {
        return None;
    }
    if hwnd == 0 {
        return None;
    }
    get_browser_url_windows(app_name, window_title, hwnd)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn resolve_browser_url_for_hwnd(app_name: &str, window_title: &str, _hwnd: isize) -> Option<String> {
    resolve_browser_url_for_window(app_name, window_title)
}

#[cfg(any(target_os = "macos", target_os = "linux", test))]
fn firefox_family_session_store_base_dir(app_lower: &str) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let app_support_dir = dirs::data_dir()?;

        if app_lower.contains("firefox") {
            Some(app_support_dir.join("Firefox"))
        } else if app_lower.contains("zen") {
            Some(app_support_dir.join("Zen"))
        } else {
            None
        }
    }

    #[cfg(target_os = "linux")]
    {
        let home_dir = dirs::home_dir()?;

        if app_lower.contains("librewolf") {
            Some(home_dir.join(".librewolf"))
        } else if app_lower.contains("waterfox") {
            Some(home_dir.join(".waterfox"))
        } else if app_lower.contains("zen") {
            Some(home_dir.join(".zen"))
        } else if app_lower.contains("firefox") {
            Some(home_dir.join(".mozilla/firefox"))
        } else {
            None
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = app_lower;
        None
    }
}

fn firefox_family_profile_dir_from_ini(base_dir: &Path, ini_content: &str) -> Option<PathBuf> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum SectionKind {
        Other,
        Install,
        Profile,
    }

    let mut section = SectionKind::Other;
    let mut install_default_path: Option<String> = None;
    let mut profile_path: Option<String> = None;
    let mut profile_is_relative = true;
    let mut profile_is_default = false;
    let mut default_profile_path: Option<String> = None;
    let mut first_profile_path: Option<String> = None;

    let finalize_profile = |profile_path: &mut Option<String>,
                            profile_is_relative: &mut bool,
                            profile_is_default: &mut bool,
                            default_profile_path: &mut Option<String>,
                            first_profile_path: &mut Option<String>| {
        let Some(path) = profile_path.take() else {
            *profile_is_relative = true;
            *profile_is_default = false;
            return;
        };

        let resolved = if *profile_is_relative {
            base_dir.join(&path)
        } else {
            PathBuf::from(&path)
        };

        if first_profile_path.is_none() {
            *first_profile_path = Some(resolved.to_string_lossy().to_string());
        }
        if *profile_is_default {
            *default_profile_path = Some(resolved.to_string_lossy().to_string());
        }

        *profile_is_relative = true;
        *profile_is_default = false;
    };

    for raw_line in ini_content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            if section == SectionKind::Profile {
                finalize_profile(
                    &mut profile_path,
                    &mut profile_is_relative,
                    &mut profile_is_default,
                    &mut default_profile_path,
                    &mut first_profile_path,
                );
            }

            let section_name = &line[1..line.len() - 1];
            section = if section_name.starts_with("Install") {
                SectionKind::Install
            } else if section_name.starts_with("Profile") {
                SectionKind::Profile
            } else {
                SectionKind::Other
            };
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match section {
            SectionKind::Install if key == "Default" => {
                install_default_path = Some(base_dir.join(value).to_string_lossy().to_string());
            }
            SectionKind::Profile => match key {
                "Path" => profile_path = Some(value.to_string()),
                "IsRelative" => profile_is_relative = value != "0",
                "Default" => profile_is_default = value == "1",
                _ => {}
            },
            SectionKind::Other | SectionKind::Install => {}
        }
    }

    if section == SectionKind::Profile {
        finalize_profile(
            &mut profile_path,
            &mut profile_is_relative,
            &mut profile_is_default,
            &mut default_profile_path,
            &mut first_profile_path,
        );
    }

    install_default_path
        .or(default_profile_path)
        .or(first_profile_path)
        .map(PathBuf::from)
}

#[cfg(any(target_os = "macos", target_os = "linux", test))]
fn firefox_family_session_store_url(app_name: &str, window_title: &str) -> Option<String> {
    let app_lower = app_name.to_lowercase();
    let base_dir = firefox_family_session_store_base_dir(&app_lower)?;
    let ini_path = base_dir.join("profiles.ini");
    let ini_content = std::fs::read_to_string(&ini_path).ok()?;
    let profile_dir = firefox_family_profile_dir_from_ini(&base_dir, &ini_content)?;

    let session_paths = [
        profile_dir.join("sessionstore-backups/recovery.jsonlz4"),
        profile_dir.join("sessionstore.jsonlz4"),
    ];

    for session_path in session_paths {
        use serde_json::Value;

        let Ok(raw) = std::fs::read(&session_path) else {
            continue;
        };
        let Ok(decoded) = rule::decode_mozlz4_bytes(&raw) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&decoded) else {
            continue;
        };
        if let Some(url) =
            rule::extract_active_tab_url_from_session_store_value(&value, window_title)
        {
            // log_browser_url_once(
            //     &format!("sessionstore:{app_name}"),
            //     &format!("从 sessionstore 获取到 {app_name} URL"),
            //     &url,
            // );
            return Some(url);
        }
    }

    None
}

/// Windows PowerShell 5.1 + UIAutomation 兜底读取真实地址栏 URL
/// 仅在原生 UIAutomation 失败时调用，避免常态化子进程开销。
#[cfg(target_os = "windows")]
fn get_url_via_powershell_uia(hwnd: isize) -> Option<String> {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const POWERSHELL_PATH: &str = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";

    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$hwnd = [IntPtr]::new({hwnd})
if ($hwnd -eq [IntPtr]::Zero) {{ exit 0 }}

$window = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
if ($null -eq $window) {{ exit 0 }}

$editCondition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
    [System.Windows.Automation.ControlType]::Edit
)
$docCondition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
    [System.Windows.Automation.ControlType]::Document
)
$allConditions = New-Object System.Windows.Automation.OrCondition($editCondition, $docCondition)
$nodes = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $allConditions)

for ($i = 0; $i -lt $nodes.Count; $i++) {{
    $node = $nodes.Item($i)
    $candidates = New-Object System.Collections.Generic.List[string]

    try {{
        $vp = $node.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        if ($vp -ne $null -and $vp.Current.Value) {{ [void]$candidates.Add($vp.Current.Value) }}
    }} catch {{ }}

    try {{
        $lp = $node.GetCurrentPattern([System.Windows.Automation.LegacyIAccessiblePattern]::Pattern)
        if ($lp -ne $null -and $lp.Current.Value) {{ [void]$candidates.Add($lp.Current.Value) }}
    }} catch {{ }}

    try {{
        if ($node.Current.Name) {{ [void]$candidates.Add($node.Current.Name) }}
    }} catch {{ }}

    foreach ($raw in $candidates) {{
        if ([string]::IsNullOrWhiteSpace($raw)) {{ continue }}
        $value = $raw.Trim()
        if ($value -match '^(https?://|chrome://|edge://|about:|file:)' -or
            $value -match '^(localhost|([a-zA-Z0-9-]+\.)+[a-zA-Z]{{2,}}|\d{{1,3}}(\.\d{{1,3}}){{3}})(:\d{{2,5}})?([/?#].*)?$') {{
            Write-Output $value
            exit 0
        }}
    }}
}}
"#
    );

    let output = run_monitor_command_with_timeout(
        Command::new(POWERSHELL_PATH)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Sta",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .creation_flags(CREATE_NO_WINDOW),
        "Windows PowerShell URL 采集",
    )
    .ok()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            log::debug!("PowerShell URL 采集失败: {}", stderr.trim());
        }
        return None;
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    rule::normalize_possible_url(&value)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn run_monitor_command_with_timeout(command: &mut Command, context: &str) -> Result<Output> {
    use std::process::Stdio;

    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| AppError::Unknown(format!("{context} 启动失败: {e}")))?;
    let started_at = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|e| AppError::Unknown(format!("{context} 读取输出失败: {e}")));
            }
            Ok(None) if started_at.elapsed() < MONITOR_COMMAND_TIMEOUT => {
                thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::Unknown(format!(
                    "{context} 执行超时（>{}ms）",
                    MONITOR_COMMAND_TIMEOUT.as_millis()
                )));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::Unknown(format!("{context} 等待进程失败: {e}")));
            }
        }
    }
}