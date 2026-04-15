use clap::Subcommand;
use std::process;

use super::helpers::print_external;

#[derive(Subcommand)]
pub(crate) enum StickiesAction {
    /// List stickies
    List,
    /// Create a sticky
    Create {
        /// Sticky title
        #[arg(long)]
        title: String,
        /// Sticky body
        #[arg(long)]
        body: String,
    },
    /// Update a sticky
    Update {
        /// Sticky id
        #[arg(long)]
        id: String,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New body
        #[arg(long)]
        body: Option<String>,
    },
    /// Delete a sticky
    Delete {
        /// Sticky id
        #[arg(long)]
        id: String,
    },
}

pub(crate) fn dispatch(action: StickiesAction) {
    match action {
        StickiesAction::List => match cueward_adapter_macos::stickies::list_stickies() {
            Ok(notes) => {
                print_external("stickies/list", &serde_json::to_string_pretty(&notes).unwrap());
                eprintln!("{} sticky note(s)", notes.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        StickiesAction::Create { title, body } => {
            match cueward_adapter_macos::stickies::create_sticky(&title, &body) {
                Ok(sticky) => {
                    let response = serde_json::json!({
                        "created": true,
                        "sticky": {
                            "id": sticky.id,
                            "title": sticky.title,
                            "body": sticky.body
                        }
                    });
                    print_external("stickies/create", &serde_json::to_string_pretty(&response).unwrap());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        StickiesAction::Update { id, title, body } => {
            match cueward_adapter_macos::stickies::update_sticky(&id, title.as_deref(), body.as_deref()) {
                Ok(sticky) => {
                    let response = serde_json::json!({
                        "updated": true,
                        "sticky": {
                            "id": sticky.id,
                            "title": sticky.title,
                            "body": sticky.body
                        }
                    });
                    print_external("stickies/update", &serde_json::to_string_pretty(&response).unwrap());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        StickiesAction::Delete { id } => {
            match cueward_adapter_macos::stickies::delete_sticky(&id) {
                Ok(()) => {
                    let response = serde_json::json!({
                        "deleted": true,
                        "id": id
                    });
                    print_external("stickies/delete", &serde_json::to_string_pretty(&response).unwrap());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
