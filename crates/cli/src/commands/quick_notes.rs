use std::process;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum QuickNotesAction {
    /// List all Quick Notes
    List,
    /// Create a new Quick Note
    Create {
        /// Note title
        #[arg(long)]
        title: String,
        /// Note body
        #[arg(long)]
        body: String,
    },
    /// Update a Quick Note's body
    Update {
        /// Note title to find
        #[arg(long)]
        title: String,
        /// New body content
        #[arg(long)]
        body: String,
    },
    /// Delete a Quick Note
    Delete {
        /// Note title to find
        #[arg(long)]
        title: String,
    },
    /// Archive a Quick Note into a regular folder and remove it from Quick Notes
    Archive {
        /// Note title to find. Must be unique among Quick Notes.
        #[arg(long)]
        title: String,
        /// Destination folder for the archived regular note
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
