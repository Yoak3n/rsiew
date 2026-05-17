use crate::{database::Database, utils::get_data_dir};
use super::out;

pub fn stats_commands(range: String, start: Option<i64>, end: Option<i64>) {
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