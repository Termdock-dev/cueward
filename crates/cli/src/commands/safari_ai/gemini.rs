use std::process;

use super::{GeminiAiAction, SafariAiAction};
use crate::commands::helpers::{
    print_external, to_adapter_gemini_mode, validate_optional_output_path,
};

pub(crate) fn dispatch(action: SafariAiAction, profile: Option<&str>) {
    match action {
        SafariAiAction::Prompt {
            prompt,
            mode,
            auto_confirm,
        } => {
            let gemini_action =
                match super::build_gemini_ai_action(mode, Some(&prompt), auto_confirm) {
                    Ok(a) => a,
                    Err(err) => {
                        eprintln!("error: {err}");
                        process::exit(1);
                    }
                };
            match gemini_action {
                GeminiAiAction::PromptOnly(prompt) => {
                    if let Err(e) = cueward_adapter_macos::safari::ensure_gemini_home(profile) {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                    match cueward_adapter_macos::safari::send_gemini_prompt(&prompt, profile) {
                        Ok(r) => {
                            print_external(
                                "safari/ai/gemini",
                                &serde_json::to_string_pretty(&r).unwrap(),
                            );
                            eprintln!("gemini response ready");
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            process::exit(1);
                        }
                    }
                }
                GeminiAiAction::ModeThenPrompt(mode, prompt) => {
                    if let Err(e) = cueward_adapter_macos::safari::prepare_gemini_mode(
                        to_adapter_gemini_mode(mode),
                        profile,
                    ) {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                    match cueward_adapter_macos::safari::send_gemini_prompt(&prompt, profile) {
                        Ok(r) => {
                            print_external(
                                "safari/ai/gemini",
                                &serde_json::to_string_pretty(&r).unwrap(),
                            );
                            eprintln!("gemini response ready");
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            process::exit(1);
                        }
                    }
                }
                GeminiAiAction::DeepResearchPlan(prompt, auto_confirm) => {
                    match cueward_adapter_macos::safari::start_gemini_deep_research(
                        &prompt,
                        auto_confirm,
                        profile,
                    ) {
                        Ok(r) => {
                            print_external(
                                "safari/ai/gemini/deep-research",
                                &serde_json::to_string_pretty(&r).unwrap(),
                            );
                            eprintln!("gemini deep research state ready");
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            process::exit(1);
                        }
                    }
                }
                GeminiAiAction::ModeOnly(_) => unreachable!(),
            }
        }
        SafariAiAction::Mode { mode } => {
            match cueward_adapter_macos::safari::prepare_gemini_mode(
                to_adapter_gemini_mode(mode),
                profile,
            ) {
                Ok(r) => {
                    println!("{}", serde_json::to_string_pretty(&r).unwrap());
                    eprintln!("gemini mode ready");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAiAction::List => {
            match cueward_adapter_macos::safari::gemini_list_conversations(profile) {
                Ok(convos) => {
                    print_external(
                        "safari/ai/gemini/list",
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
            match cueward_adapter_macos::safari::gemini_read_conversation(&url, profile) {
                Ok(r) => {
                    print_external(
                        "safari/ai/gemini/read",
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
        SafariAiAction::Poll { timeout } => {
            match cueward_adapter_macos::safari::poll_gemini_deep_research(timeout, profile) {
                Ok(r) => {
                    print_external(
                        "safari/ai/gemini/poll",
                        &serde_json::to_string_pretty(&r).unwrap(),
                    );
                    eprintln!("polled");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAiAction::SaveImages { url, output } => {
            if let Err(err) = validate_optional_output_path("--output", Some(output.as_str())) {
                eprintln!("{err}");
                process::exit(1);
            }
            match cueward_adapter_macos::safari::gemini_save_images(&url, &output, profile) {
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
        SafariAiAction::SaveMedia { url } => {
            match cueward_adapter_macos::safari::gemini_save_media(&url, profile) {
                Ok(r) => {
                    println!("{}", serde_json::to_string_pretty(&r).unwrap());
                    eprintln!("media download triggered");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
