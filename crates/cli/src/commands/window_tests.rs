use clap::Parser;

use super::{Cli, Command};
use crate::commands::window::WindowAction;

#[test]
fn parses_window_list_visibility_scope() {
    let default = Cli::try_parse_from(["cueward", "window", "list"]).expect("default list");
    assert!(matches!(
        default.command,
        Command::Window {
            action: WindowAction::List { all_spaces: false }
        }
    ));
    let all =
        Cli::try_parse_from(["cueward", "window", "list", "--all-spaces"]).expect("all spaces");
    assert!(matches!(
        all.command,
        Command::Window {
            action: WindowAction::List { all_spaces: true }
        }
    ));
}

#[test]
fn parses_window_snapshot_and_requires_exact_id() {
    assert!(Cli::try_parse_from(["cueward", "window", "snapshot"]).is_err());
    assert!(Cli::try_parse_from(["cueward", "window", "snapshot", "--id", "-1"]).is_err());
    let cli = Cli::try_parse_from([
        "cueward",
        "window",
        "snapshot",
        "--id",
        "42",
        "--ocr",
        "--output",
        "frame.png",
    ])
    .expect("snapshot");
    assert!(matches!(cli.command, Command::Window {
        action: WindowAction::Snapshot { id: 42, ocr: true, output: Some(path) }
    } if path == "frame.png"));
}

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
                    root,
                    limit,
                    depth,
                    screenshot,
                    ocr,
                },
        } => {
            assert_eq!(id, 123);
            assert_eq!(root, "0");
            assert_eq!(limit, 40);
            assert_eq!(depth, 4);
            assert!(!screenshot);
            assert!(ocr);
        }
        _ => panic!("wrong command"),
    }
}

#[test]
fn parses_window_inspection_subtree() {
    let cli = Cli::try_parse_from([
        "cueward", "window", "inspect", "--id", "42", "--root", "0.2.1", "--depth", "2",
    ])
    .expect("subtree inspection");
    assert!(matches!(cli.command, Command::Window {
        action: WindowAction::Inspect { id: 42, root, depth: 2, .. }
    } if root == "0.2.1"));
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
