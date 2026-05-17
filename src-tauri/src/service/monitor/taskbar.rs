use crate::core::error::Result;
use crate::service::rule::should_exclude_taskbar_window;
use super::normalize_display_app_name;

/// 任务栏窗口信息
#[derive(Debug, Clone)]
pub struct TaskbarWindow {
    pub app_name: String,
    pub window_title: String,
    pub exe_path: Option<String>,
    pub pid: u32,
    pub hwnd: isize,
    pub is_minimized: bool,
    pub is_pinned: bool,
}


#[cfg(target_os = "windows")]
pub fn get_taskbar_windows() -> Result<Vec<TaskbarWindow>> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use winapi::um::winuser::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
        IsIconic, GetWindowLongW, GWL_EXSTYLE
    };
    
    struct CallbackContext {
        windows: Vec<TaskbarWindow>,
    }
    
    unsafe extern "system" fn enum_window_callback(hwnd: winapi::shared::windef::HWND, lparam: isize) -> i32 {
        let context = &mut *(lparam as *mut CallbackContext);
        
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        
        let is_visible = IsWindowVisible(hwnd) != 0;
        let is_minimized = IsIconic(hwnd) != 0;
        
        if is_visible && (ex_style & 0x00000080) == 0 {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            
            if pid > 0 {
                let exe_path = super::get_process_image_path(pid);
                
                let mut title: [u16; 512] = [0; 512];
                let len = GetWindowTextW(hwnd, title.as_mut_ptr(), 512);
                let window_title = if len > 0 {
                    OsString::from_wide(&title[..len as usize])
                        .to_string_lossy()
                        .to_string()
                } else {
                    String::new()
                };
                
                if !window_title.is_empty() {
                    let raw_app_name = if let Some(path) = &exe_path {
                        std::path::Path::new(path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "Unknown".to_string())
                    } else {
                        "Unknown".to_string()
                    };

                    // 过滤系统窗口
                    let exe_lower = raw_app_name.to_lowercase();
                    let path_lower = exe_path.as_ref().map(|p| p.to_lowercase());
                    if should_exclude_taskbar_window(&exe_lower, path_lower.as_deref()) {
                        return 1;
                    }
                    
                    // 规范化应用名称（移除 .exe 后缀）
                    let name_without_ext = raw_app_name.trim_end_matches(".exe").trim_end_matches(".EXE");
                    let app_name = normalize_display_app_name(name_without_ext, &window_title);
                    
                    // 简化版本，实际项目中可以添加更复杂的固定检测逻辑
                    let is_pinned = false;
                    
                    let taskbar_window = TaskbarWindow {
                        app_name,
                        window_title,
                        exe_path,
                        pid,
                        hwnd: hwnd as isize,
                        is_minimized,
                        is_pinned,
                    };
                    
                    context.windows.push(taskbar_window);
                }
            }
        }
        
        1
    }
    
    let mut context = CallbackContext { windows: Vec::new() };
    
    unsafe {
        EnumWindows(Some(enum_window_callback), &mut context as *mut _ as isize);
    }
    
    Ok(context.windows)
}

#[cfg(not(target_os = "windows"))]
pub fn get_tray_applications() -> Result<Vec<TrayApplication>> {
    Err(AppError::Unknown("托盘程序检测仅支持Windows系统".to_string()))
}

#[cfg(not(target_os = "windows"))]
pub fn get_taskbar_windows() -> Result<Vec<TaskbarWindow>> {
    Err(AppError::Unknown("任务栏窗口检测仅支持Windows系统".to_string()))
}

