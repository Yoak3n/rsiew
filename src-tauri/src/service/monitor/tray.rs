use crate::core::error::Result;
use super::get_process_image_path;
use crate::service::rule::{is_system_core_process, should_exclude_tray_exe, should_exclude_tray_path};

/// 托盘程序（通知区域图标）信息
#[derive(Debug, Clone)]
pub struct TrayApplication {
    pub app_name: String,
    pub exe_path: Option<String>,
    pub pid: u32,
    pub is_visible: bool,
}


/// 系统托盘图标信息
#[derive(Debug, Clone)]
pub struct SystemTrayIcon {
    pub app_name: String,
    pub exe_path: Option<String>,
    pub pid: u32,
    pub hwnd: isize,
    pub window_title: String,
    pub tray_text: String,
    pub is_hidden: bool,
}


/// UIA 托盘检测结果
#[cfg(target_os = "windows")]
pub struct UiaTrayResult {
    pub tray_apps: Vec<TrayApplication>,
    /// 被过滤掉的 explorer.exe 托管的 UI 元素名称（用于调试）
    pub explorer_hosted_names: Vec<String>,
}

#[cfg(target_os = "windows")]
pub fn get_tray_applications() -> Result<Vec<TrayApplication>> {
    get_tray_applications_heuristic()
}

#[cfg(target_os = "windows")]
fn get_tray_applications_heuristic() -> Result<Vec<TrayApplication>> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::collections::HashSet;
    use winapi::shared::windef::HWND;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use winapi::um::winuser::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible, GetWindowLongW, GWL_EXSTYLE};

    // 步骤1: 枚举所有顶层窗口，收集有窗口的 PID
    // 使用 Box 避免所有权问题（EnumWindows 回调通过裸指针修改）
    let sets = Box::new((HashSet::<u32>::new(), HashSet::<u32>::new()));
    let raw_sets = Box::into_raw(sets);

    unsafe extern "system" fn enum_all_windows(hwnd: HWND, lparam: isize) -> i32 {
        let state = &mut *(lparam as *mut (HashSet<u32>, HashSet<u32>));
        let mut pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid); }
        if pid > 0 {
            state.0.insert(pid);
            let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 };
            let is_visible = unsafe { IsWindowVisible(hwnd) != 0 };

            // 隐藏窗口、工具窗口——典型的托盘程序模式
            if !is_visible || (ex_style & 0x00000080) != 0 {
                state.1.insert(pid);
            }
        }
        1
    }

    unsafe {
        EnumWindows(Some(enum_all_windows), raw_sets as isize);
    }

    let (window_pids, hidden_window_pids) = *unsafe { Box::from_raw(raw_sets) };

    // 步骤2: 枚举所有进程，筛选出有窗口的后台进程
    let mut tray_apps = Vec::new();
    let mut seen_pids = HashSet::new();

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() {
            return Ok(tray_apps);
        }

        let mut entry = std::mem::zeroed::<PROCESSENTRY32W>();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let pid = entry.th32ProcessID;
                let exe_name = OsString::from_wide(&entry.szExeFile)
                    .to_string_lossy()
                    .to_string();
                // 截断空字符（szExeFile是固定大小的WCHAR数组，可能包含尾部空字符）
                let exe_name = exe_name.trim_end_matches('\0').to_string();
                let exe_lower = exe_name.to_lowercase();

                // 仅处理有窗口的进程，且去重
                if pid > 0 && seen_pids.insert(pid) && window_pids.contains(&pid) {
                    if is_system_core_process(&exe_lower) {
                        continue;
                    }

                    let exe_path = get_process_image_path(pid);

                    // 基于可执行文件名的过滤
                    if should_exclude_tray_exe(&exe_lower) {
                        continue;
                    }

                    // 基于路径的过滤
                    if let Some(ref path) = exe_path {
                        if should_exclude_tray_path(&path.to_lowercase()) {
                            continue;
                        }
                    }

                    let app_name = if let Some(ref path) = exe_path {
                        std::path::Path::new(path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .map(|s| s.trim_end_matches(".exe").to_string())
                            .unwrap_or_else(|| exe_name.trim_end_matches(".exe").to_string())
                    } else {
                        exe_name.trim_end_matches(".exe").to_string()
                    };

                    let is_hidden = hidden_window_pids.contains(&pid);

                    tray_apps.push(TrayApplication {
                        app_name,
                        exe_path,
                        pid,
                        is_visible: !is_hidden,
                    });
                }

                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
    }

    // 对同名进程去重，选择"最重要"的 PID
    // 优先级：有可见窗口 > 仅有隐藏窗口；同级别取第一个
    let deduplicated = deduplicate_tray_apps(tray_apps);

    Ok(deduplicated)
}

/// 对同名托盘程序去重，选择最具代表性的 PID
///
/// 选择策略：
/// 1. 优先选有可见窗口的进程（通常是主进程）
/// 2. 同为可见/隐藏时，保留先出现的（通常是先启动的主进程）
fn deduplicate_tray_apps(apps: Vec<TrayApplication>) -> Vec<TrayApplication> {
    use std::collections::HashMap;

    let mut best_by_name: HashMap<String, TrayApplication> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for app in apps {
        let key = app.app_name.to_lowercase();
        match best_by_name.get(&key) {
            Some(existing) => {
                // 优先选可见窗口的进程
                if app.is_visible && !existing.is_visible {
                    best_by_name.insert(key.clone(), app);
                }
                // 同为可见或同为隐藏时，保留先出现的（不替换）
            }
            None => {
                order.push(key.clone());
                best_by_name.insert(key, app);
            }
        }
    }

    // 保持原始顺序
    order.into_iter()
        .filter_map(|key| best_by_name.remove(&key))
        .collect()
}



#[cfg(not(target_os = "windows"))]
pub fn get_system_tray_icons() -> Result<Vec<SystemTrayIcon>> {
    Err(AppError::Unknown("系统托盘图标检测仅支持Windows系统".to_string()))
}


// ==========================================
// 单测
// ==========================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_find_tray_toolbar_windows() {
        // 测试能否找到托盘工具栏窗口
        // 这个测试依赖Windows系统环境
        unsafe {
            use winapi::um::winuser::{FindWindowW, FindWindowExW};
            
            let shell_tray_wnd = FindWindowW(
                "Shell_TrayWnd\0".as_ptr() as *const u16,
                std::ptr::null()
            );
            if shell_tray_wnd.is_null() {
                println!("未找到Shell_TrayWnd窗口");
                return;
            }
            println!("找到Shell_TrayWnd: {:p}", shell_tray_wnd);
            
            let tray_notify = FindWindowExW(
                shell_tray_wnd,
                std::ptr::null_mut(),
                "TrayNotifyWnd\0".as_ptr() as *const u16,
                std::ptr::null()
            );
            if tray_notify.is_null() {
                println!("未找到TrayNotifyWnd");
                return;
            }
            println!("找到TrayNotifyWnd: {:p}", tray_notify);
            
            let sys_pager = FindWindowExW(
                tray_notify,
                std::ptr::null_mut(),
                "SysPager\0".as_ptr() as *const u16,
                std::ptr::null()
            );
            if sys_pager.is_null() {
                println!("未找到SysPager");
                return;
            }
            println!("找到SysPager: {:p}", sys_pager);
            
            let toolbar = FindWindowExW(
                sys_pager,
                std::ptr::null_mut(),
                "ToolbarWindow32\0".as_ptr() as *const u16,
                std::ptr::null()
            );
            if toolbar.is_null() {
                println!("未找到ToolbarWindow32，托盘可能使用不同的窗口结构");
            } else {
                println!("找到ToolbarWindow32: {:p}", toolbar);
            }
            
            // 检查NotifyIconOverflowWindow
            let overflow = FindWindowW(
                "NotifyIconOverflowWindow\0".as_ptr() as *const u16,
                std::ptr::null()
            );
            if !overflow.is_null() {
                println!("找到NotifyIconOverflowWindow: {:p}", overflow);
                let overflow_toolbar = FindWindowExW(
                    overflow,
                    std::ptr::null_mut(),
                    "ToolbarWindow32\0".as_ptr() as *const u16,
                    std::ptr::null()
                );
                if !overflow_toolbar.is_null() {
                    println!("找到折叠托盘ToolbarWindow32: {:p}", overflow_toolbar);
                }
            } else {
                println!("未找到NotifyIconOverflowWindow");
            }
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_tb_button_count() {
        // 测试TB_BUTTONCOUNT消息
        unsafe {
            use winapi::shared::minwindef::{DWORD, UINT};
            use winapi::um::winuser::{FindWindowW, FindWindowExW, SendMessageTimeoutW};
            
            const TB_BUTTONCOUNT: u32 = 0x0418;
            const SMTO_ABORTIFHUNG: u32 = 0x0002;
            
            let shell_tray_wnd = FindWindowW(
                "Shell_TrayWnd\0".as_ptr() as *const u16,
                std::ptr::null()
            );
            if shell_tray_wnd.is_null() {
                println!("未找到Shell_TrayWnd");
                return;
            }
            
            let tray_notify = FindWindowExW(
                shell_tray_wnd,
                std::ptr::null_mut(),
                "TrayNotifyWnd\0".as_ptr() as *const u16,
                std::ptr::null()
            );
            if tray_notify.is_null() {
                println!("未找到TrayNotifyWnd");
                return;
            }
            
            let sys_pager = FindWindowExW(
                tray_notify,
                std::ptr::null_mut(),
                "SysPager\0".as_ptr() as *const u16,
                std::ptr::null()
            );
            if sys_pager.is_null() {
                println!("未找到SysPager");
                return;
            }
            
            let toolbar = FindWindowExW(
                sys_pager,
                std::ptr::null_mut(),
                "ToolbarWindow32\0".as_ptr() as *const u16,
                std::ptr::null()
            );
            if toolbar.is_null() {
                println!("未找到ToolbarWindow32");
                return;
            }
            
            let mut button_count: DWORD = 0;
            let result = SendMessageTimeoutW(
                toolbar,
                TB_BUTTONCOUNT as UINT,
                0,
                0,
                SMTO_ABORTIFHUNG as UINT,
                2000,
                &mut button_count as *mut DWORD as *mut _
            );
            
            if result == 0 {
                println!("TB_BUTTONCOUNT超时");
            } else {
                println!("主托盘图标数量: {}", button_count);
            }
            
            // 检测并处理折叠托盘
            let overflow = FindWindowW(
                "NotifyIconOverflowWindow\0".as_ptr() as *const u16,
                std::ptr::null()
            );
            if !overflow.is_null() {
                let overflow_toolbar = FindWindowExW(
                    overflow,
                    std::ptr::null_mut(),
                    "ToolbarWindow32\0".as_ptr() as *const u16,
                    std::ptr::null()
                );
                if !overflow_toolbar.is_null() {
                    let mut overflow_count: DWORD = 0;
                    let overflow_result = SendMessageTimeoutW(
                        overflow_toolbar,
                        TB_BUTTONCOUNT as UINT,
                        0,
                        0,
                        SMTO_ABORTIFHUNG as UINT,
                        2000,
                        &mut overflow_count as *mut DWORD as *mut _
                    );
                    if overflow_result != 0 {
                        println!("折叠托盘图标数量: {}", overflow_count);
                    }
                }
            }
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_enum_tray_windows() {
        // 枚举所有托盘相关窗口的结构
        unsafe {
            use std::ffi::OsString;
            use std::os::windows::ffi::OsStringExt;
            use winapi::um::winuser::{
                FindWindowW, FindWindowExW, GetClassNameW, GetWindowTextW,
                GetWindowThreadProcessId
            };
            
            let tray_window_classes = [
                ("Shell_TrayWnd",     "任务栏"),
                ("NotifyIconOverflowWindow", "折叠托盘"),
                ("TrayNotifyWnd",     "通知区域"),
                ("SysPager",          "系统分页"),
                ("ToolbarWindow32",   "工具栏"),
                ("MSTaskSwWClass",    "任务按钮"),
                ("ReBarWindow32",     "ReBar"),
            ];
            
            for (class, desc) in &tray_window_classes {
                let class_name = format!("{}\0", class);
                let hwnd = FindWindowW(class_name.as_ptr() as *const u16, std::ptr::null());
                if !hwnd.is_null() {
                    let mut title_buf: [u16; 256] = [0; 256];
                    let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 256);
                    let title = if len > 0 {
                        OsString::from_wide(&title_buf[..len as usize])
                            .to_string_lossy()
                            .to_string()
                    } else { String::new() };
                    
                    let mut pid: u32 = 0;
                    GetWindowThreadProcessId(hwnd, &mut pid);
                    println!("[{}] 类名='{}' 标题='{}' HWND={:p} PID={}", 
                        desc, class, title, hwnd, pid);
                    
                    // 枚举子窗口
                    extern "system" fn enum_child(h: winapi::shared::windef::HWND, l: isize) -> i32 {
                        let depth = l;
                        let mut title_buf: [u16; 256] = [0; 256];
                        let len = unsafe { GetWindowTextW(h, title_buf.as_mut_ptr(), 256) };
                        let title = if len > 0 {
                            OsString::from_wide(&title_buf[..len as usize])
                                .to_string_lossy()
                                .to_string()
                        } else { String::new() };
                        
                        let mut class_buf: [u16; 256] = [0; 256];
                        let class_len = unsafe { GetClassNameW(h, class_buf.as_mut_ptr(), 256) };
                        let cls = if class_len > 0 {
                            OsString::from_wide(&class_buf[..class_len as usize])
                                .to_string_lossy()
                                .to_string()
                        } else { String::new() };
                        
                        let indent = "  ".repeat(depth as usize);
                        println!("{}+- class='{}' title='{}' HWND={:p}", indent, cls, title, h);
                        1
                    }
                    
                    let mut cur = FindWindowExW(hwnd, std::ptr::null_mut(), std::ptr::null(), std::ptr::null());
                    while !cur.is_null() {
                        let mut pid2: u32 = 0;
                        GetWindowThreadProcessId(cur, &mut pid2);
                        let mut title_buf2: [u16; 256] = [0; 256];
                        let len2 = GetWindowTextW(cur, title_buf2.as_mut_ptr(), 256);
                        let title2 = if len2 > 0 {
                            OsString::from_wide(&title_buf2[..len2 as usize])
                                .to_string_lossy()
                                .to_string()
                        } else { String::new() };
                        
                        let mut class_buf2: [u16; 256] = [0; 256];
                        let class_len2 = GetClassNameW(cur, class_buf2.as_mut_ptr(), 256);
                        let cls2 = if class_len2 > 0 {
                            OsString::from_wide(&class_buf2[..class_len2 as usize])
                                .to_string_lossy()
                                .to_string()
                        } else { String::new() };
                        println!("  子窗口: class='{}' title='{}' HWND={:p} PID={}", cls2, title2, cur, pid2);
                        cur = FindWindowExW(hwnd, cur, std::ptr::null(), std::ptr::null());
                    }
                } else {
                    println!("[{}] 类名='{}' -> 未找到", desc, class);
                }
            }
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_get_tray_applications() {
        // 完整的托盘程序获取测试
        match get_tray_applications() {
            Ok(apps) => {
                println!("\n=== 最终托盘程序获取结果 ===");
                println!("总共找到 {} 个托盘程序:", apps.len());
                for (i, app) in apps.iter().enumerate() {
                    println!("{}. {} (PID: {}, 可见: {})", 
                        i + 1, app.app_name, app.pid, app.is_visible);
                    if let Some(ref path) = app.exe_path {
                        println!("   路径: {}", path);
                    }
                }
                
                // 基本验证
                assert!(apps.len() > 0, "应该至少找到一个托盘程序");
                
                // 检查常见托盘程序
                let names: Vec<&str> = apps.iter().map(|a| a.app_name.as_str()).collect();
                println!("\n托盘程序列表: {:?}", names);
                
                // 验证每个托盘程序有PID
                for app in &apps {
                    assert!(app.pid > 0, "PID必须大于0: {}", app.app_name);
                }
            }
            Err(e) => {
                panic!("get_tray_applications 失败: {}", e);
            }
        }
    }

}
