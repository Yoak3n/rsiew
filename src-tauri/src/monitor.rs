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

#[cfg(target_os = "linux")]
pub fn get_active_window() -> Result<ActiveWindow, String> {

    let xprop_id = Command::new("xprop")
    .args(["-root","-_NET_ACTIVE_WINDOW"])
    .output()
    .map_err(|e| format!("Failed to execute xprop: {}", e))?;

    if !xprop_id.status.success() {
        return Err(format!("xprop failed: {}", String::from_utf8_lossy(&xprop_id.stderr)));
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

    for line in props_out.lines(){
        if line.starts_with("WM_CLASS"){
            if let Some(class_str) = line.split('=').nth(1){
                let parts:Vec<&str> = class_str.split(',').collect();
                let target = parts.last().unwrap_or(&parts[0]).trim();
                app_name = target.trim_matches('"').to_string()
            }
        }else if line.starts_with("_NET_WM_NAME") || (window_title.is_empty() && line.starts_with("WM_NAME")){
            if let Some(title_str) = line.split('=').nth(1){
                window_title = title_str.trim().trim_matches('"').to_string()
            }
        }
    }
    if app_name.is_empty(){
        app_name = "Unknown".to_string();
    }


    let mut exe_path= String::new();
    if let Ok(pid_output) = Command::new("xprop")
        .args(["-id",wind_id,"_NET_WM_PID"])
        .output()
    {
        let pid_str = String::from_utf8_lossy(&pid_output.stdout);
        if let Some(pid) = pid_str.split('=').nth(1).map(|s|s.trim()){
            if let Ok(path) = std::fs::read_link(format!("/proc/{}/exe",pid)){
                exe_path = path.to_string_lossy().to_string();
            }
        }
    }

    Ok(ActiveWindow{
        app_name: normalize_display_app_name(&app_name, &window_title),
        window_title,
        exe_path
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
            match c.next() { 
                None => String::new(), 
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str() 
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