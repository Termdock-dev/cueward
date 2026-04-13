use std::process;

use clap::{Subcommand, ValueEnum};

use super::helpers::{print_external, to_adapter_gemini_mode};

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum SafariAiProvider {
    Gemini,
    Chatgpt,
    Grok,
    Threads,
    X,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum GeminiMode {
    Image,
    DeepResearch,
    Video,
    Music,
}

#[derive(Subcommand)]
pub(crate) enum SafariAiAction {
    Prompt {
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        mode: Option<GeminiMode>,
        #[arg(long, default_value_t = false)]
        auto_confirm: bool,
    },
    Mode {
        mode: GeminiMode,
    },
    List,
    Read {
        url: String,
    },
    Poll {
        #[arg(long, default_value = "900")]
        timeout: u64,
    },
    SaveImages {
        url: String,
        #[arg(long, default_value = ".")]
        output: String,
    },
    SaveMedia {
        url: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GeminiAiAction {
    ModeOnly(GeminiMode),
    PromptOnly(String),
    ModeThenPrompt(GeminiMode, String),
    DeepResearchPlan(String, bool),
}

pub(crate) fn build_gemini_ai_action(
    mode: Option<GeminiMode>,
    prompt: Option<&str>,
    auto_confirm: bool,
) -> Result<GeminiAiAction, &'static str> {
    if auto_confirm && !matches!((&mode, prompt), (Some(GeminiMode::DeepResearch), Some(_))) {
        return Err("--auto-confirm requires --mode deep-research and --prompt");
    }

    match (mode, prompt) {
        (Some(GeminiMode::DeepResearch), Some(prompt)) => {
            Ok(GeminiAiAction::DeepResearchPlan(prompt.to_string(), auto_confirm))
        }
        (Some(mode), Some(prompt)) => Ok(GeminiAiAction::ModeThenPrompt(mode, prompt.to_string())),
        (Some(mode), None) => Ok(GeminiAiAction::ModeOnly(mode)),
        (None, Some(prompt)) => Ok(GeminiAiAction::PromptOnly(prompt.to_string())),
        (None, None) => Err("--mode or --prompt is required for Gemini Safari AI workflow"),
    }
}

pub(crate) fn dispatch(provider: SafariAiProvider, profile: Option<String>, action: SafariAiAction) {
    let p = profile.as_deref();
    match provider {
        SafariAiProvider::Gemini => dispatch_gemini(action, p),
        SafariAiProvider::Chatgpt => dispatch_chatgpt(action, p),
        SafariAiProvider::Grok => dispatch_grok(action, p),
        SafariAiProvider::Threads => dispatch_threads(action, p),
        SafariAiProvider::X => dispatch_x(action, p),
    }
}

fn dispatch_gemini(action: SafariAiAction, profile: Option<&str>) {
    match action {
        SafariAiAction::Prompt {
            prompt,
            mode,
            auto_confirm,
        } => {
            let gemini_action = match build_gemini_ai_action(mode, Some(&prompt), auto_confirm) {
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
                            print_external("safari/ai/gemini", &serde_json::to_string_pretty(&r).unwrap());
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
                            print_external("safari/ai/gemini", &serde_json::to_string_pretty(&r).unwrap());
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
        SafariAiAction::List => match cueward_adapter_macos::safari::gemini_list_conversations(profile) {
            Ok(convos) => {
                print_external("safari/ai/gemini/list", &serde_json::to_string_pretty(&convos).unwrap());
                eprintln!("{} conversation(s)", convos.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        SafariAiAction::Read { url } => {
            match cueward_adapter_macos::safari::gemini_read_conversation(&url, profile) {
                Ok(r) => {
                    print_external("safari/ai/gemini/read", &serde_json::to_string_pretty(&r).unwrap());
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
                    print_external("safari/ai/gemini/poll", &serde_json::to_string_pretty(&r).unwrap());
                    eprintln!("polled");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAiAction::SaveImages { url, output } => {
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

fn dispatch_chatgpt(action: SafariAiAction, profile: Option<&str>) {
    match action {
        SafariAiAction::Prompt { prompt, mode, auto_confirm } => {
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
                        print_external("safari/ai/chatgpt", &serde_json::to_string_pretty(&r).unwrap());
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
                            print_external("safari/ai/chatgpt/image", &serde_json::to_string_pretty(&r).unwrap());
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
                print_external("safari/ai/chatgpt/list", &serde_json::to_string_pretty(&convos).unwrap());
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

fn dispatch_grok(action: SafariAiAction, profile: Option<&str>) {
    match action {
        SafariAiAction::Prompt { prompt, mode, auto_confirm } => {
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
                    print_external("safari/ai/grok/list", &serde_json::to_string_pretty(&convos).unwrap());
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
                    print_external("safari/ai/grok/read", &serde_json::to_string_pretty(&r).unwrap());
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

fn dispatch_threads(action: SafariAiAction, profile: Option<&str>) {
    match action {
        SafariAiAction::List => match cueward_adapter_macos::safari::threads_extract_feed(profile) {
            Ok(posts) => {
                print_external("safari/threads/feed", &serde_json::to_string_pretty(&posts).unwrap());
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

fn dispatch_x(action: SafariAiAction, profile: Option<&str>) {
    match action {
        SafariAiAction::Prompt { prompt, mode, auto_confirm } => {
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
