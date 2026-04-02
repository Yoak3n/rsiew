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
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}

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
    user_path::add_to_user_path();
    #[cfg(all(windows, not(debug_assertions)))]
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
        }
        Some(Commands::UninstallCleanup) => {
            user_path::remove_from_user_path();
        }
        None => {
            rsiew_lib::run()
        }
    }
}