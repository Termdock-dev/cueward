use std::process;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum NotesAction {
    /// Update a note's body
    Update {
        /// Note title to find
        #[arg(long)]
        title: String,
        /// New body content
        #[arg(long)]
        body: String,
        /// Folder to search in
        #[arg(long, default_value = "Cueward")]
        folder: String,
    },
    /// Delete a note
    Delete {
        /// Note title to find
        #[arg(long)]
        title: String,
        /// Folder to search in
        #[arg(long, default_value = "Cueward")]
        folder: String,
    },
    /// Move a note to a different folder
    Move {
        /// Note title to find
        #[arg(long)]
        title: String,
        /// Source folder
        #[arg(long)]
        from: String,
        /// Destination folder
        #[arg(long)]
        to: String,
    },
}

pub(crate) fn dispatch(action: NotesAction) {
    match action {
        NotesAction::Update { title, body, folder } => {
            match cueward_adapter_macos::notes::crud::update_note(&title, &body, &folder) {
                Ok(()) => eprintln!("note updated: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        NotesAction::Delete { title, folder } => {
            match cueward_adapter_macos::notes::crud::delete_note(&title, &folder) {
                Ok(()) => eprintln!("note deleted: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        NotesAction::Move { title, from, to } => {
            match cueward_adapter_macos::notes::crud::move_note(&title, &from, &to) {
                Ok(()) => eprintln!("note moved: {title} ({from} -> {to})"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
