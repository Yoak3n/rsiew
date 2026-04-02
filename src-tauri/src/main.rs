// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version = env!("CARGO_PKG_VERSION"), about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Query work dynamics statistics
    Stats {
        /// Time range: today, week, or custom timestamps
        #[arg(value_name = "RANGE", default_value = "today")]
        range: String,

        /// Custom start timestamp (Unix seconds)
        #[arg(short = 's', long, value_name = "TIMESTAMP")]
        start: Option<i64>,

        /// Custom end timestamp (Unix seconds)
        #[arg(short = 'e', long, value_name = "TIMESTAMP")]
        end: Option<i64>,
    },
    /// Hidden command for PATH cleanup during uninstall
    #[command(hide = true)]
    UninstallCleanup,
}

fn get_data_dir() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("rsiew");
    std::fs::create_dir_all(&path).unwrap();
    path.push("rsiew.db");
    path
}

#[cfg(windows)]
fn remove_from_user_path() {
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
fn remove_from_user_path() {}

#[cfg(all(windows, not(debug_assertions)))]
fn add_to_user_path() {
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
fn add_to_user_path() {}

#[cfg(windows)]
fn attach_console() {
    use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}

fn print_console(msg: &str) {
    #[cfg(windows)]
    {
        use winapi::um::processenv::GetStdHandle;
        use winapi::um::winbase::STD_OUTPUT_HANDLE;
        use winapi::um::consoleapi::WriteConsoleW;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;

        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle != ptr::null_mut() && handle != winapi::um::handleapi::INVALID_HANDLE_VALUE {
                let msg_wide: Vec<u16> = std::ffi::OsStr::new(msg).encode_wide().chain(std::iter::once(0)).collect();
                let mut written = 0;
                WriteConsoleW(handle, msg_wide.as_ptr() as *const _, (msg_wide.len() - 1) as u32, &mut written, ptr::null_mut());
            } else {
                println!("{}", msg);
            }
        }
    }
    #[cfg(not(windows))]
    {
        println!("{}", msg);
    }
}

fn main() {
    #[cfg(all(windows, not(debug_assertions)))]
    add_to_user_path();
    attach_console();

    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            print_console(&e.render().to_string());
            return;
        }
    };

    match cli.command {
        Some(Commands::Stats { range, start, end }) => {
            let db_path = get_data_dir();
            let db = rsiew_lib::database::Database::new(&db_path).expect("Failed to open DB");

            let now = chrono::Local::now().timestamp();
            let mut start_ts;
            let mut end_ts = now;

            match range.as_str() {
                "today" => {
                    let today = chrono::Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
                    let today_local = today.and_local_timezone(chrono::Local).unwrap();
                    start_ts = today_local.timestamp();
                },
                "week" => {
                    let today = chrono::Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
                    let today_local = today.and_local_timezone(chrono::Local).unwrap();
                    start_ts = today_local.timestamp() - 7 * 24 * 3600;
                },
                _ => {
                    print_console("Unknown range. Use 'today' or 'week'\n");
                    return;
                }
            }

            if let Some(s) = start { start_ts = s; }
            if let Some(e) = end { end_ts = e; }

            match db.get_stats_by_range(start_ts, end_ts) {
                Ok(stats) => {
                    let mut output = String::new();
                    output.push_str("=== Work Dynamics Stats ===\n");
                    for stat in stats {
                        output.push_str(&format!("{}: {} seconds\n", stat.app_name, stat.duration));
                    }
                    print_console(&output);
                }
                Err(e) => {
                    print_console(&format!("Error querying stats: {}\n", e));
                }
            }
        }
        Some(Commands::UninstallCleanup) => {
            remove_from_user_path();
        }
        None => {
            // 没有子命令，运行图形界面模式
            rsiew_lib::run()
        }
    }
}