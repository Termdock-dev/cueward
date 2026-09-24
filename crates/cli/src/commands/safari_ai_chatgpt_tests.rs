use clap::Parser;

use super::safari::SafariAction;
use super::safari_ai::{GeminiMode, SafariAiAction, SafariAiProvider};
use super::{Cli, Command};

#[test]
fn cli_parses_chatgpt_prompt_effort() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "chatgpt",
        "prompt",
        "--prompt",
        "hello",
        "--effort",
        "pro",
    ])
    .expect("parse ChatGPT effort");

    assert!(matches!(
        cli.command,
        Command::Safari {
            action: SafariAction::Ai {
                provider: SafariAiProvider::Chatgpt,
                action: SafariAiAction::Prompt { effort: Some(ref value), .. },
                ..
            }
        } if value == "pro"
    ));
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
                            ..
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
fn cli_parses_chatgpt_prompt_timeout() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "ai",
        "--provider",
        "chatgpt",
        "prompt",
        "--prompt",
        "long answer",
        "--timeout",
        "900",
    ])
    .expect("parse ChatGPT prompt timeout");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Ai {
                    provider: SafariAiProvider::Chatgpt,
                    action: SafariAiAction::Prompt { timeout, .. },
                    ..
                },
        } => assert_eq!(timeout, Some(900)),
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
                            ..
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
