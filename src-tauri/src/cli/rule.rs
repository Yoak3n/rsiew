use crate::config::rule::RuleConfig;
use clap::Subcommand;
#[derive(Subcommand, Debug)]
pub enum RuleCommands {
    /// List all rules
    List,
    /// Add a description rule
    AddDescription {
        /// App name to match (fuzzy)
        #[arg(short = 'n', long)]
        app_name: String,
        /// Display name override
        #[arg(short = 'd', long)]
        display_name: Option<String>,
        /// Description text
        #[arg(short = 'D', long)]
        description: Option<String>,
    },
    /// Add a category rule
    AddCategory {
        /// App name to match (fuzzy)
        #[arg(short = 'n', long)]
        app_name: String,
        /// Category key
        #[arg(short = 'c', long)]
        category: String,
    },
    /// Remove a description rule
    RmDescription {
        /// App name to remove
        app_name: String,
    },
    /// Remove a category rule
    RmCategory {
        /// App name to remove
        app_name: String,
    },
    /// Update a description rule
    SetDescription {
        /// App name to update
        #[arg(short = 'n', long)]
        app_name: String,
        /// New display name
        #[arg(short = 'd', long)]
        display_name: Option<String>,
        /// New description text
        #[arg(short = 'D', long)]
        description: Option<String>,
    },
    /// Update a category rule
    SetCategory {
        /// App name to update
        #[arg(short = 'n', long)]
        app_name: String,
        /// New category key
        #[arg(short = 'c', long)]
        category: String,
    },
}



/// CLI 入口：处理 rule 子命令
pub fn handle_rule_command(command: RuleCommands) {
    let mut rules = RuleConfig::load();
    let result = match command {
        RuleCommands::List => {
            rules.list();
            Ok(())
        }
        RuleCommands::AddDescription { app_name, display_name, description } => {
            rules.add_description(app_name, display_name, description)
        }
        RuleCommands::AddCategory { app_name, category } => {
            rules.add_category(app_name, category)
        }
        RuleCommands::RmDescription { app_name } => {
            rules.remove_description(&app_name)
        }
        RuleCommands::RmCategory { app_name } => {
            rules.remove_category(&app_name)
        }
        RuleCommands::SetDescription { app_name, display_name, description } => {
            rules.update_description(&app_name, display_name, description)
        }
        RuleCommands::SetCategory { app_name, category } => {
            rules.update_category(&app_name, category)
        }
    };
    if let Err(e) = result {
        eprintln!("错误: {}", e);
        std::process::exit(1);
    }
}