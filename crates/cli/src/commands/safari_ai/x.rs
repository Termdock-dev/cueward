use std::process;

use crate::commands::helpers::print_external;

use super::SafariAiAction;

pub(crate) fn dispatch(action: SafariAiAction, profile: Option<&str>) {
    match action {
        SafariAiAction::Prompt {
            prompt,
            mode,
            auto_confirm,
        } => {
            if auto_confirm {
                eprintln!("error: X prompt does not support --auto-confirm");
                process::exit(1);
            }
            if mode.is_some() {
                eprintln!("error: X prompt does not support --mode");
                process::exit(1);
            }
            match cueward_adapter_macos::safari::x_search(&prompt, profile) {
                Ok(posts) => {
                    print_external("safari/x/search", &serde_json::to_string_pretty(&posts).unwrap());
                    eprintln!("{} post(s)", posts.len());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAiAction::List => match cueward_adapter_macos::safari::x_extract_feed(profile) {
            Ok(posts) => {
                print_external("safari/x/feed", &serde_json::to_string_pretty(&posts).unwrap());
                eprintln!("{} post(s)", posts.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        SafariAiAction::Read { url } => match cueward_adapter_macos::safari::x_read_post(&url, profile) {
            Ok(posts) => {
                print_external("safari/x/read", &serde_json::to_string_pretty(&posts).unwrap());
                eprintln!("{} post(s)", posts.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        _ => {
            eprintln!("error: X currently supports prompt, list, and read");
            process::exit(1);
        }
    }
}
