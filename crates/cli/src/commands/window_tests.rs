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
