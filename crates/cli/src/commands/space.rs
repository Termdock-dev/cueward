use clap::Subcommand;
use serde::Serialize;

use super::helpers::print_external;

#[derive(Subcommand)]
pub(crate) enum SpaceAction {
    /// List existing macOS Spaces and background window-move availability.
    List,
    /// Read the current Space membership of a titled application window.
    Window {
        #[arg(long)]
        id: u32,
    },
    /// Move a background window to an existing inactive user Space.
    MoveWindow {
        /// input_target from a fresh window snapshot.
        #[arg(long)]
        target: String,
        /// An inactive user Space id from space list.
        #[arg(long)]
        space: u64,
    },
}

pub(crate) fn dispatch(action: SpaceAction) {
    use cueward_adapter_macos::window::{list_spaces, move_window_to_space, window_spaces};
    match action {
        SpaceAction::List => output("space/list", list_spaces()),
        SpaceAction::Window { id } => output("space/window", window_spaces(id)),
        SpaceAction::MoveWindow { target, space } => {
            output("space/move-window", move_window_to_space(&target, space))
        }
    }
}

fn output<T: Serialize>(source: &str, result: Result<T, cueward_adapter_macos::MacosError>) {
    let result = result
        .map_err(|e| e.to_string())
        .and_then(|value| serde_json::to_string_pretty(&value).map_err(|e| e.to_string()));
    match result {
        Ok(payload) => print_external(source, &payload),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{Cli, Command};
    use clap::Parser;
    #[test]
    fn parses_space_discovery_membership_and_guarded_move() {
        assert!(matches!(
            Cli::try_parse_from(["cueward", "space", "list"])
                .expect("list")
                .command,
            Command::Space {
                action: SpaceAction::List
            }
        ));
        assert!(matches!(
            Cli::try_parse_from(["cueward", "space", "window", "--id", "42"])
                .expect("window")
                .command,
            Command::Space {
                action: SpaceAction::Window { id: 42 }
            }
        ));
        assert!(
            matches!(Cli::try_parse_from(["cueward", "space", "move-window", "--target", "token", "--space", "7"]).expect("move").command,
            Command::Space { action: SpaceAction::MoveWindow { target, space: 7 } } if target == "token")
        );
        assert!(Cli::try_parse_from(["cueward", "space", "move-window", "--space", "7"]).is_err());
        assert!(
            Cli::try_parse_from([
                "cueward",
                "space",
                "move-window",
                "--target",
                "token",
                "--space",
                "-1"
            ])
            .is_err()
        );
    }
}
