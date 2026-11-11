use std::process;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum NotesAction {
    /// Create a note
    Create {
        /// Note title
        #[arg(long)]
        title: String,
        /// Body content
        #[arg(long)]
        body: String,
        /// Target folder
        #[arg(long, default_value = "Cueward")]
        folder: String,
    },
    /// Update a note's body
    Update {
        /// Note title to find
        #[arg(long, required_unless_present = "id", conflicts_with = "id")]
        title: Option<String>,
        /// Native Notes ID from capture metadata.note_id
        #[arg(long, required_unless_present = "title", conflicts_with = "title")]
        id: Option<String>,
        /// New body content
        #[arg(long)]
        body: String,
        /// Folder to search in
        #[arg(long, default_value = "Cueward", conflicts_with = "id")]
        folder: String,
    },
    /// Delete a note
    Delete {
        /// Note title to find
        #[arg(long, required_unless_present = "id", conflicts_with = "id")]
        title: Option<String>,
        /// Native Notes ID from capture metadata.note_id
        #[arg(long, required_unless_present = "title", conflicts_with = "title")]
        id: Option<String>,
        /// Folder to search in
        #[arg(long, default_value = "Cueward", conflicts_with = "id")]
        folder: String,
    },
    /// Move a note to a different folder
    Move {
        /// Note title to find
        #[arg(long, required_unless_present = "id", conflicts_with = "id")]
        title: Option<String>,
        /// Native Notes ID from capture metadata.note_id
        #[arg(long, required_unless_present = "title", conflicts_with = "title")]
        id: Option<String>,
        /// Source folder
        #[arg(long, required_unless_present = "id", conflicts_with = "id")]
        from: Option<String>,
        /// Destination folder
        #[arg(long)]
        to: String,
    },
}

pub(crate) fn dispatch(action: NotesAction) {
    match action {
        NotesAction::Create {
            title,
            body,
            folder,
        } => match cueward_adapter_macos::notes::crud::create_note(&title, &body, &folder) {
            Ok(()) => eprintln!("note created: {title}"),
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        NotesAction::Update {
            title,
            id,
            body,
            folder,
        } => {
            match match (title.as_deref(), id.as_deref()) {
                (Some(title), None) => {
                    cueward_adapter_macos::notes::crud::update_note(title, &body, &folder)
                }
                (None, Some(id)) => {
                    cueward_adapter_macos::notes::crud::update_note_by_id(id, &body)
                }
                _ => unreachable!("clap validates the selector"),
            } {
                Ok(()) => eprintln!("note updated"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        NotesAction::Delete { title, id, folder } => {
            match match (title.as_deref(), id.as_deref()) {
                (Some(title), None) => {
                    cueward_adapter_macos::notes::crud::delete_note(title, &folder)
                }
                (None, Some(id)) => cueward_adapter_macos::notes::crud::delete_note_by_id(id),
                _ => unreachable!("clap validates the selector"),
            } {
                Ok(()) => eprintln!("note deleted"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        NotesAction::Move {
            title,
            id,
            from,
            to,
        } => {
            match match (title.as_deref(), id.as_deref(), from.as_deref()) {
                (Some(title), None, Some(from)) => {
                    cueward_adapter_macos::notes::crud::move_note(title, from, &to)
                }
                (None, Some(id), None) => {
                    cueward_adapter_macos::notes::crud::move_note_by_id(id, &to)
                }
                _ => unreachable!("clap validates the selector"),
            } {
                Ok(()) => eprintln!("note moved to {to}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
