use super::extract;
use super::super::monitor;
use super::normalize;

struct WindowsSystemDialogRule {
    executable_names: &'static [&'static str],
    exact_window_texts: &'static [&'static str],
}

const WINDOWS_SYSTEM_DIALOG_RULES: &[WindowsSystemDialogRule] = &[
    WindowsSystemDialogRule {
        executable_names: &["taskmgr"],
        exact_window_texts: &[
            "task manager",
            "task manager (not responding)",
            "任务管理器",
            "任务管理器 (未响应)",
            "任务管理器（未响应）",
        ],
    },
    WindowsSystemDialogRule {
        executable_names: &["consent", "credentialuibroker"],
        exact_window_texts: &[
            "user account control",
            "windows security",
            "用户账户控制",
            "用户帐户控制",
            "windows 安全",
            "windows 安全中心",
        ],
    },
];

pub fn is_system_process(app_name: &str) -> bool {
    let name_lower = app_name.to_lowercase();
    let name_lower = name_lower.trim_end_matches(".exe");

    matches!(
        name_lower,
        // Windows 桌面 / 锁屏 / 搜索
        "desktop"
            | "lockapp"
            | "logonui"
            | "searchapp"
            | "searchhost"
            | "shellexperiencehost"
            | "startmenuexperiencehost"
            | "textinputhost"
            | "applicationframehost"
            | "dwm"
            | "csrss"
            | "taskmgr"
            // macOS 桌面 / 锁屏
            | "loginwindow"
            | "screensaverengine"
            | "screensaver"
            // Linux 桌面 / 锁屏 / 系统进程
            | "cinnamon-session"
            | "cinnamon-screensaver"
            | "gnome-shell"
            | "gnome-screensaver"
            | "plasmashell"
            | "kscreenlocker"
            | "xscreensaver"
            | "i3lock"
            | "swaylock"
            | "xfce4-session"
    )
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn is_probable_host(value: &str) -> bool {
    let host = value.trim().trim_end_matches('.');
    if host.is_empty() {
        return false;
    }

    let (host_without_port, _) = extract::split_host_port(host);
    let host_lower = host_without_port.to_lowercase();

    host_lower == "localhost"
        || is_probable_domain(host_without_port)
        || is_probable_ipv4(host_without_port)
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn is_probable_ipv4(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() != 4 {
        return false;
    }

    parts.iter().all(|part| {
        !part.is_empty()
            && part.len() <= 3
            && part.chars().all(|c| c.is_ascii_digit())
            && part.parse::<u8>().is_ok()
    })
}


#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn is_probable_domain(value: &str) -> bool {
    let candidate = value.trim().trim_matches('/').to_lowercase();
    if candidate.is_empty()
        || candidate.contains(' ')
        || candidate.starts_with('.')
        || candidate.ends_with('.')
        || !candidate.contains('.')
    {
        return false;
    }

    let labels: Vec<&str> = candidate.split('.').collect();
    if labels.len() < 2 {
        return false;
    }

    let tld = labels.last().copied().unwrap_or_default();
    // TLD 最少 2 字符、最多 12 字符，且必须全是 ASCII 字母
    // 上限防止 OCR 丢失斜杠后把域名和路径拼为超长假 TLD（如 github.comwm94i）
    if tld.len() < 2 || tld.len() > 12 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }

    labels.iter().all(|label| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}


/// Detect host-only domains that are likely OCR slash-loss artifacts
/// e.g. `linux.do/latest` → OCR loses `/` → `linux.dolatest`
pub fn is_merged_domain(url: &str) -> bool {
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);

    let (host, rest) = extract::split_host_and_rest(without_scheme);
    if !rest.is_empty() {
        return false;
    }

    let host = extract::split_host_port(host).0.trim_end_matches('.');
    if host.is_empty() || host == "localhost" {
        return false;
    }

    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() != 2 {
        return false;
    }

    let tld = labels[1].to_lowercase();
    if tld.len() <= 6 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }

    let prefix = &tld[..2];
    matches!(
        prefix,
        "ai" | "cc"
            | "cn"
            | "de"
            | "do"
            | "fr"
            | "hk"
            | "id"
            | "in"
            | "io"
            | "jp"
            | "kr"
            | "me"
            | "ru"
            | "sg"
            | "tv"
            | "uk"
            | "us"
    )
}


#[cfg(any(target_os = "macos", target_os = "linux", test))]
#[allow(dead_code)]
pub fn matches_firefox_family_browser(app_lower: &str) -> bool {
    app_lower.contains("firefox")
        || app_lower.contains("zen")
        || app_lower.contains("librewolf")
        || app_lower.contains("waterfox")
}

/// 判断进程名是否属于浏览器
pub fn is_browser_app(app_name: &str) -> bool {
    let app_lower = app_name.to_lowercase();
    let substring_match = app_lower.contains("chrome")
        || app_lower.contains("msedge")
        || app_lower.contains("microsoft edge")
        || app_lower.contains("brave")
        || app_lower.contains("opera")
        || app_lower.contains("vivaldi")
        || app_lower.contains("firefox")
        || app_lower.contains("safari")
        || app_lower.contains("orion")
        || app_lower.contains("zen browser")
        || app_lower.contains("browser")
        || app_lower.contains("qq browser")
        || app_lower.contains("360 browser")
        || app_lower.contains("sogou browser")
        || app_lower.contains("360se")
        || app_lower.contains("360chrome")
        || app_lower.contains("qqbrowser")
        || app_lower.contains("sogouexplorer")
        || app_lower.contains("2345explorer")
        || app_lower.contains("liebao")
        || app_lower.contains("maxthon")
        || app_lower.contains("theworld")
        || app_lower.contains("iexplore");
    if substring_match {
        return true;
    }
    // 与 work_review_core::categorize::is_browser_app 保持一致：
    //   "cent" / "arc" 用精确匹配，避免 "Tencent Lemon" / "Arch Linux" 等被误判为浏览器
    matches!(
        app_lower.as_str(),
        "cent" | "cent browser" | "centbrowser" | "arc"
    )
}


fn matches_windows_system_dialog_rule(
    active_window: &monitor::ActiveWindow,
    rule: &WindowsSystemDialogRule,
) -> bool {
    let executable_name = extract::windows_executable_name(active_window);
    if executable_name.as_deref().is_some_and(|name| {
        rule.executable_names
            .iter()
            .any(|candidate| candidate == &name)
    }) {
        return true;
    }

    let app_name = normalize::normalized_windows_system_window_text(&active_window.app_name);
    let window_title = normalize::normalized_windows_system_window_text(&active_window.window_title);
    let allow_exact_text_fallback =
        !app_name.is_empty() && (window_title.is_empty() || app_name == window_title);

    allow_exact_text_fallback
        && rule
            .exact_window_texts
            .iter()
            .any(|candidate| candidate == &app_name)
}

pub fn is_windows_system_dialog(active_window: &monitor::ActiveWindow) -> bool {
    WINDOWS_SYSTEM_DIALOG_RULES
        .iter()
        .any(|rule| matches_windows_system_dialog_rule(active_window, rule))
}



/// 判断 UIA 元素名称是否是系统 UI 元素（非真正的托盘图标）
///
/// 这些元素由 explorer.exe 托管，属于任务栏固定的系统图标或按钮，
/// 不应被识别为用户安装的托盘程序。
pub fn is_system_tray_element(name_lower: &str) -> bool {
    // 任务栏固定按钮
    if matches!(
        name_lower,
        "start"
            | "开始"
            | "search"
            | "搜索"
            | "task view"
            | "任务视图"
            | "widgets"
            | "小工具"
            | "chat"
            | "聊天"
            | "copilot"
            | "show hidden icons"
            | "显示隐藏的图标"
            | "notification chevron"
            | "通知区域展开"
            | "action center"
            | "操作中心"
            | "touch keyboard"
            | "触摸键盘"
            | "meet now"
            | "立即开会"
            | "focus assist"
            | "专注助手"
    ) {
        return true;
    }

    // 系统固定图标关键词
    let system_keywords = [
        "weather", "天气",
        "news", "新闻",
        "clock", "时钟",
        "date", "日期",
        "battery", "电池",
        "network", "网络",
        "volume", "音量",
        "speaker", "扬声器",
        "wifi", "bluetooth", "蓝牙",
        "power", "电源",
        "display", "显示器",
        "language", "语言",
    ];

    for keyword in &system_keywords {
        if name_lower.contains(keyword) {
            return true;
        }
    }

    false
}

/// 判断名称是否是 explorer.exe 托管的系统 shell 元素（任务栏按钮等）
///
/// 用于在通过 UIA 原生 PID 检测时，排除不属于真实托盘程序的 UI 元素。
pub fn is_explorer_shell_element(name_lower: &str) -> bool {
    matches!(
        name_lower,
        "start"
            | "开始"
            | "search"
            | "搜索"
            | "task view"
            | "任务视图"
            | "widgets"
            | "小工具"
            | "chat"
            | "聊天"
            | "copilot"
            | "show hidden icons"
            | "显示隐藏的图标"
            | "notification chevron"
            | "通知区域展开"
            | "action center"
            | "操作中心"
            | "touch keyboard"
            | "触摸键盘"
            | "meet now"
            | "立即开会"
            | "focus assist"
            | "专注助手"
    ) || name_lower.starts_with("microsoft ")
        || name_lower.starts_with("windows ")
}

// ============================================================
//  启发式托盘检测 —— 进程级过滤
// ============================================================

/// 判断进程名是否是 Windows 系统核心进程（不应出现在托盘列表中）
pub fn is_system_core_process(exe_lower: &str) -> bool {
    matches!(
        exe_lower,
        "system"
            | "system idle process"
            | "explorer.exe"
            | "svchost.exe"
            | "csrss.exe"
            | "smss.exe"
            | "wininit.exe"
            | "services.exe"
            | "lsass.exe"
            | "fontdrvhost.exe"
            | "winlogon.exe"
            | "sihost.exe"
            | "taskhostw.exe"
            | "dwm.exe"
            | "ctfmon.exe"
    ) || exe_lower.starts_with("runtimebroker")
        || exe_lower.starts_with("backgroundtaskhost")
}