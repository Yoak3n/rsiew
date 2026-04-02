

#[cfg(windows)]
pub fn remove_from_user_path() {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_dir_str = exe_dir.to_string_lossy().to_string();
            
            use winreg::enums::*;
            use winreg::RegKey;
            use winapi::um::winuser::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG};
            use std::os::windows::ffi::OsStrExt;

            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            if let Ok((env, _)) = hkcu.create_subkey("Environment") {
                let current_path: String = env.get_value("Path").unwrap_or_default();
                
                if current_path.contains(&exe_dir_str) {
                    let mut paths: Vec<&str> = current_path.split(';').collect();
                    paths.retain(|p| *p != exe_dir_str);
                    let new_path = paths.join(";");
                    
                    let _ = env.set_value("Path", &new_path);
                    
                    // Broadcast environment change
                    unsafe {
                        use winapi::shared::minwindef::{WPARAM, LPARAM};
                        let env_str: Vec<u16> = std::ffi::OsStr::new("Environment").encode_wide().chain(std::iter::once(0)).collect();
                        SendMessageTimeoutW(
                            HWND_BROADCAST,
                            0x001A, // WM_SETTINGCHANGE
                            0 as WPARAM,
                            env_str.as_ptr() as LPARAM,
                            SMTO_ABORTIFHUNG,
                            5000,
                            std::ptr::null_mut()
                        );
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub fn remove_from_user_path() {}



#[cfg(all(windows, not(debug_assertions)))]
pub fn add_to_user_path() {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_dir_str = exe_dir.to_string_lossy().to_string();
            
            // Only add if it's installed in Program Files or AppData (avoid adding dev folder)
            if exe_dir_str.contains("rsiew") {
                use winreg::enums::*;
                use winreg::RegKey;
                use winapi::um::winuser::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG};
                use std::os::windows::ffi::OsStrExt;

                let hkcu = RegKey::predef(HKEY_CURRENT_USER);
                if let Ok((env, _)) = hkcu.create_subkey("Environment") {
                    let current_path: String = env.get_value("Path").unwrap_or_default();
                    
                    if !current_path.contains(&exe_dir_str) {
                        let new_path = if current_path.ends_with(';') || current_path.is_empty() {
                            format!("{}{}", current_path, exe_dir_str)
                        } else {
                            format!("{};{}", current_path, exe_dir_str)
                        };
                        
                        let _ = env.set_value("Path", &new_path);
                        
                        // Broadcast environment change
                        unsafe {
                            use winapi::shared::minwindef::{WPARAM, LPARAM};
                            let env_str: Vec<u16> = std::ffi::OsStr::new("Environment").encode_wide().chain(std::iter::once(0)).collect();
                            SendMessageTimeoutW(
                                HWND_BROADCAST,
                                0x001A, // WM_SETTINGCHANGE
                                0 as WPARAM,
                                env_str.as_ptr() as LPARAM,
                                SMTO_ABORTIFHUNG,
                                5000,
                                std::ptr::null_mut()
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub fn add_to_user_path() {}