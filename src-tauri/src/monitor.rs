#[cfg(not(target_os = "windows"))]
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ActiveWindow {
    pub app_name: String,
    pub window_title: String,
    pub exe_path: String, // 增加这个，以便稍后提取图标
}

// ==========================================
// Windows 实现 (WinAPI)
// ==========================================
#[cfg(target_os = "windows")]
pub fn get_active_window() -> Result<ActiveWindow, String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winbase::QueryFullProcessImageNameW;
    use winapi::um::winuser::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };
    
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return Err("No foreground window (hwnd is null)".to_string());
        }

        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let window_title = if len > 0 {
            OsString::from_wide(&title_buf[..len as usize]).into_string().unwrap_or_default()
        } else {
            String::new()
        };

        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut process_id);

        if process_id == 0 {
            return Err("Failed to get process ID".to_string());
        }

        let mut app_name = format!("PID_{}", process_id);
        let mut exe_path = String::new();
        let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        
        if !process_handle.is_null() {
            let mut name_buf = [0u16; 1024];
            let mut name_len = name_buf.len() as u32;
            
            let success = QueryFullProcessImageNameW(
                process_handle,
                0,
                name_buf.as_mut_ptr(),
                &mut name_len,
            );

            if success != 0 && name_len > 0 {
                if let Ok(full_path) = OsString::from_wide(&name_buf[..name_len as usize]).into_string() {
                    exe_path = full_path.clone();
                    if let Some(file_name) = std::path::Path::new(&full_path).file_name() {
                        app_name = file_name.to_string_lossy().trim_end_matches(".exe").to_string();
                    }
                }
            }
            CloseHandle(process_handle);
        }

        Ok(ActiveWindow {
            app_name: normalize_display_app_name(&app_name, &window_title),
            window_title,
            exe_path,
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_active_window() -> Result<ActiveWindow, String> {
    Ok(ActiveWindow {
        app_name: normalize_display_app_name("MockApp", "MockTitle"),
        window_title: "MockTitle".to_string(),
        exe_path: "/mock/path/MockApp.exe".to_string(),
    })
}

pub fn normalize_display_app_name(raw_name: &str, window_title: &str) -> String {
    let lower_name = raw_name.to_lowercase();
    let lower_title = window_title.to_lowercase();
    if lower_name.contains("electron") || lower_name.contains("helper") {
        if lower_title.contains("visual studio code") || lower_title.contains("vs code") { return "VS Code".to_string(); }
        if lower_title.contains("discord") { return "Discord".to_string(); }
        if lower_title.contains("figma") { return "Figma".to_string(); }
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
            match c.next() { None => String::new(), Some(f) => f.to_uppercase().collect::<String>() + c.as_str() }
        }
    }
}