use clap::Subcommand;
use std::process;
use std::str::FromStr;

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
        /// Target display number
        #[arg(long)]
        display: Option<u32>,
        /// Sticky color preset
        #[arg(long)]
        color: Option<String>,
        /// X coordinate
        #[arg(long, requires = "y")]
        x: Option<i32>,
        /// Y coordinate
        #[arg(long, requires = "x")]
        y: Option<i32>,
        /// Sticky width
        #[arg(long, requires = "height")]
        width: Option<i32>,
        /// Sticky height
        #[arg(long, requires = "width")]
        height: Option<i32>,
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
        /// Target display number
        #[arg(long)]
        display: Option<u32>,
        /// Sticky color preset
        #[arg(long)]
        color: Option<String>,
        /// X coordinate
        #[arg(long, requires = "y")]
        x: Option<i32>,
        /// Y coordinate
        #[arg(long, requires = "x")]
        y: Option<i32>,
        /// Sticky width
        #[arg(long, requires = "height")]
        width: Option<i32>,
        /// Sticky height
        #[arg(long, requires = "width")]
        height: Option<i32>,
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
        StickiesAction::Create {
            title,
            body,
            display,
            color,
            x,
            y,
            width,
            height,
        } => {
            let color = match color {
                Some(value) => match cueward_adapter_macos::stickies::StickyColorPreset::from_str(&value) {
                    Ok(color) => Some(color),
                    Err(err) => {
                        eprintln!("error: {err}");
                        process::exit(1);
                    }
                },
                None => None,
            };
            let options = cueward_adapter_macos::stickies::StickyMutationOptions {
                color,
                display,
                x,
                y,
                width,
                height,
            };
            match cueward_adapter_macos::stickies::create_sticky_with_options(&title, &body, &options) {
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
        StickiesAction::Update {
            id,
            title,
            body,
            display,
            color,
            x,
            y,
            width,
            height,
        } => {
            let color = match color {
                Some(value) => match cueward_adapter_macos::stickies::StickyColorPreset::from_str(&value) {
                    Ok(color) => Some(color),
                    Err(err) => {
                        eprintln!("error: {err}");
                        process::exit(1);
                    }
                },
                None => None,
            };
            let options = cueward_adapter_macos::stickies::StickyMutationOptions {
                color,
                display,
                x,
                y,
                width,
                height,
            };
            match cueward_adapter_macos::stickies::update_sticky_with_options(
                &id,
                title.as_deref(),
                body.as_deref(),
                &options,
            ) {
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
