use std::process;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum NotesAction {
    Update {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: String,
        #[arg(long, default_value = "Cueward")]
        folder: String,
    },
    Delete {
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "Cueward")]
        folder: String,
    },
    Move {
        #[arg(long)]
        title: String,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
    },
}

pub(crate) fn dispatch(action: NotesAction) {
    match action {
        NotesAction::Update { title, body, folder } => {
            match cueward_adapter_macos::send::update_note(&title, &body, &folder) {
                Ok(()) => eprintln!("note updated: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        NotesAction::Delete { title, folder } => {
            match cueward_adapter_macos::send::delete_note(&title, &folder) {
                Ok(()) => eprintln!("note deleted: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        NotesAction::Move { title, from, to } => {
            match cueward_adapter_macos::send::move_note(&title, &from, &to) {
                Ok(()) => eprintln!("note moved: {title} ({from} -> {to})"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
