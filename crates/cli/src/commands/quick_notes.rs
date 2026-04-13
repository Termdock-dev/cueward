use std::process;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum QuickNotesAction {
    List,
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: String,
    },
    Update {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: String,
    },
    Delete {
        #[arg(long)]
        title: String,
    },
    Archive {
        #[arg(long)]
        title: String,
        #[arg(long)]
        to: String,
    },
}

pub(crate) fn dispatch(action: QuickNotesAction) {
    match action {
        QuickNotesAction::List => match cueward_adapter_macos::quick_notes::list() {
            Ok(notes) => {
                if notes.is_empty() {
                    eprintln!("no quick notes found");
                } else {
                    let count = notes.len();
                    println!("{}", serde_json::to_string_pretty(&notes).unwrap());
                    eprintln!("{count} quick note(s)");
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        QuickNotesAction::Create { title, body } => {
            match cueward_adapter_macos::quick_notes::create(&title, &body) {
                Ok(()) => eprintln!("quick note created: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        QuickNotesAction::Update { title, body } => {
            match cueward_adapter_macos::quick_notes::update(&title, &body) {
                Ok(()) => eprintln!("quick note updated: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        QuickNotesAction::Delete { title } => {
            match cueward_adapter_macos::quick_notes::delete(&title) {
                Ok(()) => eprintln!("quick note deleted: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        QuickNotesAction::Archive { title, to } => {
            match cueward_adapter_macos::quick_notes::archive(&title, &to) {
                Ok(()) => eprintln!("quick note archived: {title} -> {to}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
