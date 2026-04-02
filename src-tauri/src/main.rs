// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod user_path;

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





#[cfg(all(windows, not(debug_assertions)))]
fn attach_console() {
    use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::{STD_OUTPUT_HANDLE, STD_ERROR_HANDLE};
    use winapi::um::fileapi::CreateFileW;
    use winapi::um::winnt::{FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};
    use winapi::um::fileapi::OPEN_EXISTING;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);

        // Redirect stdout and stderr to the console
        let con_out_name: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
        let con_out = CreateFileW(
            con_out_name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut()
        );

        if con_out != winapi::um::handleapi::INVALID_HANDLE_VALUE {
            use winapi::um::processenv::SetStdHandle;
            SetStdHandle(STD_OUTPUT_HANDLE, con_out);
            SetStdHandle(STD_ERROR_HANDLE, con_out);
        }
    }
}

#[cfg(not(windows))]
fn attach_console() {}



 #[cfg(windows)]
fn detach_and_send_enter() {
    use winapi::um::wincon::{FreeConsole, WriteConsoleInputW, INPUT_RECORD, KEY_EVENT};
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::STD_INPUT_HANDLE;
    use winapi::um::winuser::VK_RETURN;
    use std::ptr;
 
    unsafe {
        let stdin = GetStdHandle(STD_INPUT_HANDLE);
        if stdin != ptr::null_mut() && stdin != winapi::um::handleapi::INVALID_HANDLE_VALUE {
            let mut record: INPUT_RECORD = std::mem::zeroed();
            record.EventType = KEY_EVENT;
            {
                let key_event = record.Event.KeyEvent_mut();
                key_event.bKeyDown = 1;
                key_event.wVirtualKeyCode = VK_RETURN as u16;
                key_event.wVirtualScanCode = 0x1C;
                *key_event.uChar.UnicodeChar_mut() = '\r' as u16;
                key_event.dwControlKeyState = 0;
            }
 
            let mut written = 0;
            WriteConsoleInputW(stdin, &mut record, 1, &mut written);
 
            {
                let key_event = record.Event.KeyEvent_mut();
                key_event.bKeyDown = 0;
            }
            WriteConsoleInputW(stdin, &mut record, 1, &mut written);
        }
        FreeConsole();
    }

}

#[cfg(not(windows))]
fn detach_and_send_enter() {}

fn format_duration(seconds: i64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    if h > 0 {
        format!("{}h {}m", h, m)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len - 1).collect::<String>() + "…"
    }
}

fn print_console(msg: &str) {
       use std::io::Write;
    #[cfg(windows)]
    {
        let formatted = msg.replace("\r\n", "\n").replace('\n', "\r\n");
        print!("{}", formatted);
    }
    #[cfg(not(windows))]
    {
        print!("{}", msg);
    }
    let _ = std::io::stdout().flush();
}

fn main() {
    #[cfg(all(windows, not(debug_assertions)))]
    user_path::add_to_user_path();
    #[cfg(all(windows, not(debug_assertions)))]
    attach_console();

    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            print_console(&e.render().to_string());
            detach_and_send_enter();
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
                    let total_seconds: i64 = stats.iter().map(|s| s.duration).sum();

                    let mut output = String::new();
                    output.push_str("=========================================================\n");
                    output.push_str("      Work Dynamics Stats\n");
                    output.push_str("=========================================================\n");

                    if stats.is_empty() {
                        output.push_str("No activity recorded.\n");
                    } else {
                        output.push_str(&format!("{:<30} | {:>10} | {:>8}\n", "App Name", "Duration", "%"));
                        output.push_str("--------------------------------------------------------\n");

                        for stat in &stats {
                            let percentage = if total_seconds > 0 {
                                (stat.duration as f64 / total_seconds as f64) * 100.0
                            } else {
                                0.0
                            };
                            let duration_str = format_duration(stat.duration);
                            let app_name = truncate_string(&stat.app_name, 30);
                            output.push_str(&format!(
                                "{:<30} | {:>10} | {:>7.1}%\n",
                                app_name, duration_str, percentage
                            ));
                        }

                        output.push_str("--------------------------------------------------------\n");
                        output.push_str(&format!("Total: {:>38}\n", format_duration(total_seconds)));
                    }

                    output.push_str("=========================================================\n");
                    print_console(&output);
                }
                Err(e) => {
                    print_console(&format!("Error querying stats: {}\n", e));
                }
            }
            detach_and_send_enter();
        }
        Some(Commands::UninstallCleanup) => {
            user_path::remove_from_user_path();
        }
        None => {
            rsiew_lib::run()
        }
    }
}