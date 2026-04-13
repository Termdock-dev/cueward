use std::process;

use crate::commands::helpers::print_external;

use super::SafariAiAction;

pub(crate) fn dispatch(action: SafariAiAction, profile: Option<&str>) {
    match action {
        SafariAiAction::List => match cueward_adapter_macos::safari::threads_extract_feed(profile) {
            Ok(posts) => {
                print_external(
                    "safari/threads/feed",
                    &serde_json::to_string_pretty(&posts).unwrap(),
                );
                eprintln!("{} post(s)", posts.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        _ => {
            eprintln!("error: Threads currently supports only list");
            process::exit(1);
        }
    }
}
