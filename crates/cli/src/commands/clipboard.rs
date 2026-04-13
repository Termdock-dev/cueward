use std::process;

use clap::Subcommand;

use super::helpers::{print_external, validate_optional_output_path};

#[derive(Subcommand)]
pub(crate) enum ClipboardAction {
    Get {
        #[arg(long)]
        save_image: Option<String>,
    },
    Set {
        text: String,
    },
}

pub(crate) fn dispatch(action: ClipboardAction) {
    match action {
        ClipboardAction::Get { save_image } => {
            if let Err(err) = validate_optional_output_path("--save-image", save_image.as_deref()) {
                eprintln!("{err}");
                process::exit(1);
            }
            match cueward_adapter_macos::clipboard::get(save_image.as_deref()) {
                Ok(content) => {
                    print_external(
                        "clipboard/get",
                        &serde_json::to_string_pretty(&content).unwrap(),
                    );
                    match content.content_type.as_str() {
                        "image" => eprintln!(
                            "clipboard image saved to {}",
                            content.path.unwrap_or_default()
                        ),
                        _ => eprintln!("clipboard text read"),
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        ClipboardAction::Set { text } => match cueward_adapter_macos::clipboard::set(&text) {
            Ok(()) => eprintln!("copied to clipboard"),
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
    }
}
