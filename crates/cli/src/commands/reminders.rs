use std::process;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum RemindersAction {
    /// List reminders, optionally filtered by list name
    List {
        /// Filter by reminders list name
        #[arg(long)]
        list: Option<String>,
    },
    /// List reminders due today
    Today,
}

pub(crate) fn dispatch_plan(title: String, notes: String, list: String) {
    match cueward_adapter_macos::plan::create_reminder(&title, &notes, &list) {
        Ok(()) => eprintln!("reminder created in {list}"),
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}

pub(crate) fn dispatch(action: RemindersAction) {
    match action {
        RemindersAction::List { list } => {
            match cueward_adapter_macos::reminders::list(list.as_deref()) {
                Ok(reminders) => {
                    println!("{}", serde_json::to_string_pretty(&reminders).unwrap());
                    eprintln!("{} reminder(s)", reminders.len());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        RemindersAction::Today => match cueward_adapter_macos::reminders::today() {
            Ok(reminders) => {
                println!("{}", serde_json::to_string_pretty(&reminders).unwrap());
                eprintln!("{} reminder(s) due today", reminders.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
    }
}
