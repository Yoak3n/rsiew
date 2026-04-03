mod out;

use crate::{database::Database, user_path, utils::get_data_dir};
use clap::{Parser, Subcommand};

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

pub fn parse_as_cli() -> bool {
    let cli = Cli::try_parse().unwrap();
    match cli.command {
        Some(Commands::Stats { range, start, end }) => {
            stats_commands(range, start, end);
            true
        }
        Some(Commands::UninstallCleanup) => {
            user_path::remove_from_user_path();
            true
        }
        None => false,
    }
}

fn stats_commands(range: String, start: Option<i64>, end: Option<i64>) {
    let db_path = get_data_dir();
    let db = Database::new(&db_path).expect("Failed to open DB");

    let now = chrono::Local::now().timestamp();
    let mut start_ts;
    let mut end_ts = now;

    match range.as_str() {
        "today" => {
            let today = chrono::Local::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let today_local = today.and_local_timezone(chrono::Local).unwrap();
            start_ts = today_local.timestamp();
        }
        "week" => {
            let today = chrono::Local::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let today_local = today.and_local_timezone(chrono::Local).unwrap();
            start_ts = today_local.timestamp() - 7 * 24 * 3600;
        }
        _ => {
            println!("Unknown range. Use 'today' or 'week'");
            return;
        }
    }

    if let Some(s) = start {
        start_ts = s;
    }
    if let Some(e) = end {
        end_ts = e;
    }

    match db.get_stats_by_range(start_ts, end_ts) {
        Ok(stats) => out::print_stats(stats),
        Err(e) => {
            println!("Error querying stats: {}", e);
        }
    }
}
