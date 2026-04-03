use crate::database::AppUsage;

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


pub fn print_stats(usages: Vec<AppUsage>) {
    let total_seconds: i64 = usages.iter().map(|s| s.duration).sum();

    let mut output = String::new();
    output.push_str("=========================================================\n");
    output.push_str("      Work Dynamics Stats\n");
    output.push_str("=========================================================\n");

    if usages.is_empty() {
        output.push_str("No activity recorded.\n");
    } else {
        output.push_str(&format!(
            "{:<30} | {:>10} | {:>8}\n",
            "App Name", "Duration", "%"
        ));
        output.push_str("--------------------------------------------------------\n");

        for usage in &usages {
            let percentage = if total_seconds > 0 {
                (usage.duration as f64 / total_seconds as f64) * 100.0
            } else {
                0.0
            };
            let duration_str = format_duration(usage.duration);
            let app_name = truncate_string(&usage.app_name, 30);
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
