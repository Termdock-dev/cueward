use clap::Subcommand;
use serde::Serialize;
use std::path::PathBuf;

use super::helpers::print_external;

#[derive(Subcommand)]
pub(crate) enum AppAction {
    /// List running regular and accessory applications.
    List,
    /// Request a background launch, or return an existing app without reopening it.
    Launch {
        #[arg(long, required_unless_present = "path", conflicts_with = "path")]
        bundle: Option<String>,
        #[arg(long, required_unless_present = "bundle", conflicts_with = "bundle")]
        path: Option<PathBuf>,
    },
}

fn output<T: Serialize>(source: &str, result: Result<T, cueward_adapter_macos::MacosError>) {
    match result
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::to_string_pretty(&v).map_err(|e| e.to_string()))
    {
        Ok(payload) => print_external(source, &payload),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

pub(crate) fn dispatch(action: AppAction) {
    use cueward_adapter_macos::apps::{launch_app, list_apps};
    match action {
        AppAction::List => output("app/list", list_apps()),
        AppAction::Launch { bundle, path } => {
            output("app/launch", launch_app(bundle.as_deref(), path.as_deref()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{Cli, Command};
    use clap::Parser;
    #[test]
    fn parses_app_discovery_and_exact_launch_selector() {
        assert!(matches!(
            Cli::try_parse_from(["cueward", "app", "list"])
                .expect("list")
                .command,
            Command::App {
                action: AppAction::List
            }
        ));
        assert!(matches!(
            Cli::try_parse_from(["cueward", "app", "launch", "--bundle", "org.example.App"])
                .expect("bundle")
                .command,
            Command::App {
                action: AppAction::Launch {
                    bundle: Some(_),
                    path: None
                }
            }
        ));
        assert!(matches!(
            Cli::try_parse_from(["cueward", "app", "launch", "--path", "/tmp/Fixture.app"])
                .expect("path")
                .command,
            Command::App {
                action: AppAction::Launch {
                    bundle: None,
                    path: Some(_)
                }
            }
        ));
        assert!(Cli::try_parse_from(["cueward", "app", "launch"]).is_err());
        assert!(
            Cli::try_parse_from([
                "cueward",
                "app",
                "launch",
                "--path",
                "/tmp/Fixture.app",
                "--bundle",
                "org.example.App"
            ])
            .is_err()
        );
    }
}
