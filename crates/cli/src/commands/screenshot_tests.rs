use clap::Parser;

use super::{Cli, Command};
use super::screenshot::ScreenshotAction;

#[test]
fn cli_parses_screenshot_capture_defaults() {
    let cli = Cli::try_parse_from(["cueward", "screenshot"]).expect("parse screenshot");

    match cli.command {
        Command::Screenshot { action: None, .. } => {}
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_screenshot_windows() {
    let cli = Cli::try_parse_from(["cueward", "screenshot", "windows"])
        .expect("parse screenshot windows");

    match cli.command {
        Command::Screenshot {
            action: Some(ScreenshotAction::Windows),
            ..
        } => {}
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_screenshot_window_id_with_ocr_and_output() {
    let cli = Cli::try_parse_from([
        "cueward",
        "screenshot",
        "window",
        "--id",
        "12345",
        "--ocr",
        "--output",
        "out.png",
    ])
    .expect("parse screenshot window");

    match cli.command {
        Command::Screenshot {
            action: Some(ScreenshotAction::Window { id, ocr, output }),
            ..
        } => {
            assert_eq!(id, 12345);
            assert!(ocr);
            assert_eq!(output.as_deref(), Some("out.png"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_screenshot_window_with_parent_flags() {
    let cli = Cli::try_parse_from([
        "cueward",
        "screenshot",
        "--ocr",
        "window",
        "--id",
        "12345",
    ])
    .expect("parse screenshot window with parent flags");

    match cli.command {
        Command::Screenshot {
            ocr,
            action: Some(ScreenshotAction::Window { id, ocr: inner_ocr, .. }),
            ..
        } => {
            assert!(ocr);
            assert_eq!(id, 12345);
            assert!(!inner_ocr);
        }
        _ => panic!("unexpected command"),
    }
}
