use std::process;

use clap::ValueEnum;

use super::{GeminiMode, SafariAiAction};
use crate::commands::helpers::print_external;

pub(crate) fn dispatch(action: SafariAiAction, profile: Option<&str>) {
    match action {
        SafariAiAction::Prompt {
            prompt,
            mode,
            auto_confirm,
        } => {
            if auto_confirm {
                eprintln!("error: ChatGPT prompt does not support --auto-confirm");
                process::exit(1);
            }
            if let Err(e) = cueward_adapter_macos::safari::ensure_chatgpt_home(profile) {
                eprintln!("error: {e}");
                process::exit(1);
            }
            match mode {
                None => match cueward_adapter_macos::safari::send_chatgpt_prompt(&prompt, profile) {
                    Ok(r) => {
                        print_external(
                            "safari/ai/chatgpt",
                            &serde_json::to_string_pretty(&r).unwrap(),
                        );
                        eprintln!("chatgpt response ready");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                },
                Some(GeminiMode::Image) => {
                    match cueward_adapter_macos::safari::send_chatgpt_image_prompt(&prompt, profile) {
                        Ok(r) => {
                            print_external(
                                "safari/ai/chatgpt/image",
                                &serde_json::to_string_pretty(&r).unwrap(),
                            );
                            eprintln!("chatgpt image response ready");
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            process::exit(1);
                        }
                    }
                }
                Some(other) => {
                    let mode_name = other
                        .to_possible_value()
                        .map(|v| v.get_name().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    eprintln!("error: ChatGPT prompt does not support --mode {} yet", mode_name);
                    process::exit(1);
                }
            }
        }
        SafariAiAction::SaveImages { url, output } => {
            match cueward_adapter_macos::safari::chatgpt_save_images(&url, &output, profile) {
                Ok(paths) => {
                    println!("{}", serde_json::to_string_pretty(&paths).unwrap());
                    eprintln!("{} image(s) saved", paths.len());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAiAction::List => match cueward_adapter_macos::safari::chatgpt_list_conversations(profile) {
            Ok(convos) => {
                print_external(
                    "safari/ai/chatgpt/list",
                    &serde_json::to_string_pretty(&convos).unwrap(),
                );
                eprintln!("{} conversation(s)", convos.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        _ => {
            eprintln!("error: ChatGPT currently supports prompt, list, and save-images");
            process::exit(1);
        }
    }
}
