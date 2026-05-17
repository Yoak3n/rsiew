mod out;
mod watch;
mod rule;
mod stats;

use stats::stats_commands;
use watch::watch_commands;
use rule::{handle_rule_command, RuleCommands};

use clap::{error::ErrorKind, Parser, Subcommand};


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
    /// Watch the system dynamics in real-time with various filters
    Watch {
        /// Watch work dynamics in real-time
        #[arg(short = 'a', long)]
        all: bool,
        /// Watch only one process
        #[arg(short = 'o', long)]
        only: bool,
        /// Watch tray applications
        #[arg(short = 't', long)]
        tray: bool,
        /// Watch taskbar windows
        #[arg(short = 'b', long)]
        taskbar: bool,
        /// Ignore specific process names
        #[arg(short = 'i', long, value_name = "APP_NAME")]
        ignore: Option<String>,
    },
    /// Manage rules (description and category)
    Rule {
        #[command(subcommand)]
        command: RuleCommands,
    },
    /// Hidden command for PATH cleanup during uninstall
    #[command(hide = true)]
    UninstallCleanup,
}

pub fn parse_as_cli() -> bool {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => match e.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                e.print().expect("Error writing to stdout");
                std::process::exit(0);
            }
            ErrorKind::MissingSubcommand => return false,
            _ => {
                e.print().expect("Error writing to stdout");
                std::process::exit(1);
            }
        },
    };
    match cli.command {
        Some(Commands::Stats { range, start, end }) => {
            stats_commands(range, start, end);
            true
        }
        Some(Commands::Watch {
            all,
            only,
            tray,
            taskbar,
            ignore,
        }) => {
            watch_commands(all, only, tray, taskbar, ignore);
            true
        }
        Some(Commands::Rule { command }) => {
            handle_rule_command(command);
            true
        }
        Some(Commands::UninstallCleanup) => {
            #[cfg(all(windows, not(debug_assertions)))]
            user_path::remove_from_user_path();
            true
        }
        None => false,
    }
}



