use crate::service::monitor;
use super::{is_browser_app,is_system_process,is_windows_system_dialog};

pub fn should_skip_system_window(active_window: &monitor::ActiveWindow) -> bool {
    let is_sys = is_system_process(&active_window.app_name);
    let is_minimized_window = active_window.is_minimized;
    let is_explorer_shell = {
        let name_lower = active_window.app_name.to_lowercase();
        let name_trimmed = name_lower.trim_end_matches(".exe");
        (name_trimmed == "explorer" || name_trimmed == "file explorer")
            && active_window.window_title.is_empty()
    };
    // Windows 在 UAC / 任务管理器异常时，进程名可能退化成标题或受保护进程名，
    // 需要结合标题与可执行路径一起兜底过滤。
    let is_windows_system_dialog = is_windows_system_dialog(active_window);
    
    is_sys || is_minimized_window || is_explorer_shell || is_windows_system_dialog
}


pub fn should_refresh_browser_url_before_record(app_name: &str, window_title: &str) -> bool {
    is_browser_app(app_name) && !window_title.is_empty()
}



/// 判断可执行文件名是否属于不应计入托盘的程序
///
/// 涵盖：浏览器/WebView、控制台、游戏平台、崩溃报告、系统服务、
///       GPU 相关、模拟器、常见非托盘后台程序等。
pub fn should_exclude_tray_exe(exe_lower: &str) -> bool {
    // 浏览器和 Web 组件
    if exe_lower == "msedge.exe"
        || exe_lower == "chrome.exe"
        || exe_lower == "firefox.exe"
        || exe_lower.contains("webview")
        || exe_lower.contains("webhelper")
    {
        return true;
    }

    // 控制台和命令行工具
    if exe_lower == "conhost.exe"
        || exe_lower == "cmd.exe"
        || exe_lower == "powershell.exe"
        || exe_lower == "pwsh.exe"
    {
        return true;
    }

    // Steam 和游戏平台
    if exe_lower == "steam.exe"
        || exe_lower.contains("steamwebhelper")
        || exe_lower == "games.exe"
    {
        return true;
    }

    // 崩溃报告和调试工具（排除精确匹配 "debug"，避免误杀含 debug 的合法应用）
    if exe_lower.contains("crashpad")
        || exe_lower.contains("crashhandler")
        || exe_lower.contains("crashreporter")
    {
        return true;
    }

    // ADB 和其他开发工具
    if exe_lower == "adb.exe" || exe_lower.contains("vctip") {
        return true;
    }

    // 排除自身
    if exe_lower == "rsiew.exe" {
        return true;
    }

    // 开发工具
    if exe_lower == "cargo.exe" || exe_lower == "rustup.exe" || exe_lower == "rustc.exe" {
        return true;
    }

    // 系统服务和主机进程
    if exe_lower == "shellhost.exe" || exe_lower == "textinputhost.exe" {
        return true;
    }

    // NVIDIA 相关进程
    if exe_lower.contains("nvdisplay")
        || exe_lower.contains("nvcontainer")
        || exe_lower.contains("nvidia")
        || exe_lower.contains("nvsphelper")
    {
        return true;
    }

    // 模拟器和游戏服务
    if exe_lower.contains("mumu")
        || exe_lower.contains("bluestacks")
        || exe_lower.contains("nox")
        || exe_lower.contains("ldplayer")
    {
        return true;
    }

    // 其他常见的非托盘进程
    if exe_lower == "sunshine.exe" || exe_lower.contains("razer") {
        return true;
    }

    // MSI Center（需要同时包含 msi 和 center，避免误杀 MSI 商标的应用）
    if exe_lower.contains("msi") && exe_lower.contains("center") {
        return true;
    }

    // Windows 跨设备服务、你的手机助手、触摸键盘、安全中心
    if exe_lower.contains("crossdevice")
        || exe_lower.contains("phoneexperience")
        || exe_lower.contains("tabtip")
        || exe_lower.contains("securityhealthsystray")
        || exe_lower.contains("wallpaper32")
    {
        return true;
    }

    false
}

/// 判断进程可执行文件路径是否属于系统/不应计入托盘的目录
pub fn should_exclude_tray_path(path_lower: &str) -> bool {
    path_lower.contains("\\windows\\system32")
        || path_lower.contains("\\windows\\syswow64")
        || path_lower.contains("\\program files (x86)\\microsoft\\edgewebview")
        || path_lower.contains("\\windowsapps\\")
        || path_lower.contains("\\systemapps\\")
        || path_lower.contains("\\immersivecontrolpanel\\")
        || path_lower.contains("\\common files\\")
        || path_lower.contains("\\ide\\")
        || (path_lower.contains("\\microsoft\\") && path_lower.contains("\\windows\\"))
}

// ============================================================
//  任务栏窗口过滤
// ============================================================

/// 判断窗口是否属于不应出现在任务栏窗口列表中的系统窗口
///
/// 包含：系统设置容器、UWP 应用宿主、输入法宿主、锁屏、桌面窗口等。
pub fn should_exclude_taskbar_window(exe_lower: &str, path_lower: Option<&str>) -> bool {
    // UWP 应用宿主（ApplicationFrameHost 托管所有 UWP 窗口，本身不是用户程序）
    if exe_lower == "applicationframehost.exe" {
        return true;
    }

    // 输入法/输入体验
    if exe_lower == "textinputhost.exe" || exe_lower == "tabtip.exe" {
        return true;
    }

    // Windows 锁屏和桌面
    if exe_lower == "lockapp.exe" || exe_lower == "explorer.exe" {
        return true;
    }

    // 系统设置（由 ImmersiveControlPanel 路径过滤即可，但兜底处理）
    if exe_lower == "systemsettings.exe" {
        return true;
    }

    // 基于路径的系统组件过滤
    if let Some(path) = path_lower {
        if path.contains("\\systemapps\\") || path.contains("\\immersivecontrolpanel\\") {
            return true;
        }
    }

    false
}


pub fn should_skip_transient_window(active_window: &monitor::ActiveWindow) -> bool {
    let app_lower = active_window.app_name.to_lowercase();
    matches!(
        app_lower.as_str(),
        "dock"
            | "systemuiserver"
            | "control center"
            | "spotlight"
            | "notificationcenter"
            | "loginwindow"
            | "screencaptureui"
            | "universalaccessauthwarn"
            | "windowmanager"
            | "wallpaper"
    )
}
