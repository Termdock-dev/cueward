use clap::Parser;

use super::{Cli, Command};
use crate::commands::window::WindowAction;

#[test]
fn parses_window_inspection_with_image_options() {
    let cli = Cli::try_parse_from([
        "cueward", "window", "inspect", "--id", "123", "--limit", "40", "--depth", "4", "--ocr",
    ])
    .expect("parse window inspection");

    match cli.command {
        Command::Window {
            action:
                WindowAction::Inspect {
                    id,
                    limit,
                    depth,
                    screenshot,
                    ocr,
                },
        } => {
            assert_eq!(id, 123);
            assert_eq!(limit, 40);
            assert_eq!(depth, 4);
            assert!(!screenshot);
            assert!(ocr);
        }
        _ => panic!("wrong command"),
    }
}

#[test]
fn parses_window_press_target() {
    let cli = Cli::try_parse_from(["cueward", "window", "press", "--target", "token"])
        .expect("parse window press");
    assert!(matches!(cli.command, Command::Window {
        action: WindowAction::Press { target }
    } if target == "token"));
}

#[test]
fn parses_window_set_value_with_leading_hyphen() {
    let cli = Cli::try_parse_from([
        "cueward",
        "window",
        "set-value",
        "--target",
        "token",
        "--value",
        "--draft",
    ])
    .expect("parse window set value");
    assert!(matches!(cli.command, Command::Window {
        action: WindowAction::SetValue { target, value }
    } if target == "token" && value == "--draft"));
}
