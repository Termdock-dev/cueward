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
                eprintln!("error: Grok prompt does not support --auto-confirm");
                process::exit(1);
            }
            if mode.is_some() {
                eprintln!("error: Grok prompt does not support --mode yet");
                process::exit(1);
            }
            if let Err(e) = cueward_adapter_macos::safari::ensure_grok_home(profile) {
                eprintln!("error: {e}");
                process::exit(1);
            }
            match cueward_adapter_macos::safari::send_grok_prompt(&prompt, profile) {
                Ok(r) => {
                    print_external("safari/ai/grok", &serde_json::to_string_pretty(&r).unwrap());
                    eprintln!("grok response ready");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAiAction::List => {
            if let Err(e) = cueward_adapter_macos::safari::ensure_grok_home(profile) {
                eprintln!("error: {e}");
                process::exit(1);
            }
            match cueward_adapter_macos::safari::grok_list_conversations(profile) {
                Ok(convos) => {
                    print_external(
                        "safari/ai/grok/list",
                        &serde_json::to_string_pretty(&convos).unwrap(),
                    );
                    eprintln!("{} conversation(s)", convos.len());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAiAction::Read { url } => {
            match cueward_adapter_macos::safari::grok_read_conversation(&url, profile) {
                Ok(r) => {
                    print_external(
                        "safari/ai/grok/read",
                        &serde_json::to_string_pretty(&r).unwrap(),
                    );
                    eprintln!("conversation read");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("error: Grok currently supports only prompt, list, and read");
            process::exit(1);
        }
    }
}
