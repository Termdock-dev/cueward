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

#[test]
fn parses_background_text_and_requires_target() {
    assert!(Cli::try_parse_from(["cueward", "window", "type-text", "--text", "hello"]).is_err());
    let cli = Cli::try_parse_from([
        "cueward",
        "window",
        "type-text",
        "--target",
        "snapshot",
        "--text",
        "--draft",
    ])
    .expect("background text");
    assert!(matches!(cli.command, Command::Window {
        action: WindowAction::TypeText { target, text }
    } if target == "snapshot" && text == "--draft"));
}

#[test]
fn parses_background_key_modifiers() {
    let cli = Cli::try_parse_from([
        "cueward",
        "window",
        "key",
        "--target",
        "snapshot",
        "--key",
        "tab",
        "--modifiers",
        "shift,option",
    ])
    .expect("background key");
    assert!(matches!(cli.command, Command::Window {
        action: WindowAction::Key { key, modifiers, .. }
    } if key == "tab" && modifiers == ["shift", "option"]));
}

#[test]
fn parses_background_scroll_point_and_signed_deltas() {
    let cli = Cli::try_parse_from([
        "cueward",
        "window",
        "scroll",
        "--target",
        "snapshot",
        "--x",
        "320.5",
        "--y",
        "100",
        "--delta-y",
        "-240",
    ])
    .expect("background scroll");
    assert!(matches!(
        cli.command,
        Command::Window {
            action: WindowAction::Scroll {
                x: 320.5,
                y: 100.0,
                delta_x: 0,
                delta_y: -240,
                ..
            }
        }
    ));
}

#[test]
fn parses_background_input_status() {
    assert!(Cli::try_parse_from(["cueward", "window", "input-status"]).is_err());
    let cli = Cli::try_parse_from(["cueward", "window", "input-status", "--target", "snapshot"])
        .expect("status");
    assert!(
        matches!(cli.command, Command::Window { action: WindowAction::InputStatus { target } } if target == "snapshot")
    );
}

#[test]
fn parses_background_click_button_and_count() {
    let cli = Cli::try_parse_from([
        "cueward", "window", "click", "--target", "snapshot", "--x", "100", "--y", "200",
        "--button", "right", "--count", "2",
    ])
    .expect("click");
    assert!(
        matches!(cli.command, Command::Window { action: WindowAction::Click { x: 100.0, y: 200.0, button, count: 2, .. } } if button == "right")
    );
}

#[test]
fn parses_background_drag_endpoints_and_duration() {
    let cli = Cli::try_parse_from([
        "cueward",
        "window",
        "drag",
        "--target",
        "snapshot",
        "--x",
        "100",
        "--y",
        "200",
        "--to-x",
        "400",
        "--to-y",
        "500",
        "--duration-ms",
        "800",
    ])
    .expect("drag");
    assert!(matches!(
        cli.command,
        Command::Window {
            action: WindowAction::Drag {
                x: 100.0,
                y: 200.0,
                to_x: 400.0,
                to_y: 500.0,
                duration_ms: 800,
                ..
            }
        }
    ));
}
