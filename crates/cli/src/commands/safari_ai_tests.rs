use clap::Parser;

use super::{Cli, Command};
use super::safari::SafariAction;
use super::safari_ai::{GeminiAiAction, GeminiMode, SafariAiAction, SafariAiProvider, build_gemini_ai_action};

#[test]
fn build_gemini_ai_action_rejects_invalid_auto_confirm_usage() {
    let result = build_gemini_ai_action(Some(GeminiMode::Image), Some("hello"), true);

    assert_eq!(
        result,
        Err("--auto-confirm requires --mode deep-research and --prompt")
    );
}

#[test]
fn build_gemini_ai_action_allows_mode_and_prompt_together() {
    let result = build_gemini_ai_action(Some(GeminiMode::Image), Some("hello"), false);

    assert_eq!(
        result,
        Ok(GeminiAiAction::ModeThenPrompt(
            GeminiMode::Image,
            "hello".to_string()
        ))
    );
}

#[test]
fn cli_parses_gemini_prompt_with_mode() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "gemini",
        "prompt",
        "--prompt",
        "研究議題",
        "--mode",
        "deep-research",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action:
                        SafariAiAction::Prompt {
                            prompt,
                            mode,
                            auto_confirm,
                        },
                    ..
                },
        } => {
            assert_eq!(provider, SafariAiProvider::Gemini);
            assert_eq!(prompt, "研究議題");
            assert_eq!(mode, Some(GeminiMode::DeepResearch));
            assert!(!auto_confirm);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_gemini_prompt_only() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "gemini",
        "prompt",
        "--prompt",
        "哈囉",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    action: SafariAiAction::Prompt { prompt, mode, .. },
                    ..
                },
        } => {
            assert_eq!(prompt, "哈囉");
            assert_eq!(mode, None);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_chatgpt_prompt_only() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "chatgpt",
        "prompt",
        "--prompt",
        "哈囉 ChatGPT",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action:
                        SafariAiAction::Prompt {
                            prompt,
                            mode,
                            auto_confirm,
                        },
                    ..
                },
        } => {
            assert_eq!(provider, SafariAiProvider::Chatgpt);
            assert_eq!(prompt, "哈囉 ChatGPT");
            assert_eq!(mode, None);
            assert!(!auto_confirm);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_chatgpt_prompt_with_image_mode() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "chatgpt",
        "prompt",
        "--prompt",
        "畫一隻貓",
        "--mode",
        "image",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action:
                        SafariAiAction::Prompt {
                            prompt,
                            mode,
                            auto_confirm,
                        },
                    ..
                },
        } => {
            assert_eq!(provider, SafariAiProvider::Chatgpt);
            assert_eq!(prompt, "畫一隻貓");
            assert_eq!(mode, Some(GeminiMode::Image));
            assert!(!auto_confirm);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_gemini_auto_confirm() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "gemini",
        "prompt",
        "--prompt",
        "研究議題",
        "--mode",
        "deep-research",
        "--auto-confirm",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    action:
                        SafariAiAction::Prompt {
                            auto_confirm, ..
                        },
                    ..
                },
        } => assert!(auto_confirm),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_gemini_list() {
    let cli = Cli::try_parse_from(["cueward", "safari", "ai", "--provider", "gemini", "list"])
        .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action: SafariAiAction::List,
                    ..
                },
        } => assert_eq!(provider, SafariAiProvider::Gemini),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_threads_list() {
    let cli = Cli::try_parse_from(["cueward", "safari", "ai", "--provider", "threads", "list"])
        .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action: SafariAiAction::List,
                    ..
                },
        } => assert_eq!(provider, SafariAiProvider::Threads),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_gemini_with_profile() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "gemini",
        "--profile",
        "Work",
        "prompt",
        "--prompt",
        "hello",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    profile,
                    action: SafariAiAction::Prompt { prompt, .. },
                    ..
                },
        } => {
            assert_eq!(profile.as_deref(), Some("Work"));
            assert_eq!(prompt, "hello");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_gemini_poll() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "gemini",
        "poll",
        "--timeout",
        "30",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action: SafariAiAction::Poll { timeout },
                    ..
                },
        } => {
            assert_eq!(provider, SafariAiProvider::Gemini);
            assert_eq!(timeout, 30);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_grok_prompt_only() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "grok",
        "prompt",
        "--prompt",
        "哈囉 Grok",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action:
                        SafariAiAction::Prompt {
                            prompt,
                            mode,
                            auto_confirm,
                        },
                    ..
                },
        } => {
            assert_eq!(provider, SafariAiProvider::Grok);
            assert_eq!(prompt, "哈囉 Grok");
            assert_eq!(mode, None);
            assert!(!auto_confirm);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_grok_list() {
    let cli = Cli::try_parse_from(["cueward", "safari", "ai", "--provider", "grok", "list"])
        .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action: SafariAiAction::List,
                    ..
                },
        } => assert_eq!(provider, SafariAiProvider::Grok),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_grok_read() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "grok",
        "read",
        "https://grok.com/c/abc",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action: SafariAiAction::Read { url },
                    ..
                },
        } => {
            assert_eq!(provider, SafariAiProvider::Grok);
            assert_eq!(url, "https://grok.com/c/abc");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_chatgpt_list() {
    let cli = Cli::try_parse_from(["cueward", "safari", "ai", "--provider", "chatgpt", "list"])
        .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action: SafariAiAction::List,
                    ..
                },
        } => assert_eq!(provider, SafariAiProvider::Chatgpt),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_x_prompt() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "x",
        "prompt",
        "--prompt",
        "台灣 AI",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action: SafariAiAction::Prompt { prompt, .. },
                    ..
                },
        } => {
            assert_eq!(provider, SafariAiProvider::X);
            assert_eq!(prompt, "台灣 AI");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_x_read() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "x",
        "read",
        "https://x.com/example/status/1",
    ])
    .expect("parse");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider,
                    action: SafariAiAction::Read { url },
                    ..
                },
        } => {
            assert_eq!(provider, SafariAiProvider::X);
            assert_eq!(url, "https://x.com/example/status/1");
        }
        _ => panic!("unexpected command"),
    }
}
