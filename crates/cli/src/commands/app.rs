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
    /// Discover an app's AX roots, or inspect an observed subtree.
    Inspect {
        #[arg(long)]
        pid: i32,
        /// Ref returned by app inspect, such as w0, w0.1, or menu.
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long, default_value_t = 3)]
        depth: usize,
    },
    /// Press an element observed through app inspect.
    Press {
        #[arg(long)]
        target: String,
    },
    /// Replace an observed app text field's AX value and read it back.
    SetValue {
        #[arg(long)]
        target: String,
        #[arg(long, allow_hyphen_values = true)]
        value: String,
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
    use cueward_adapter_macos::apps::{
        inspect_app, launch_app, list_apps, press_app_element, set_app_value,
    };
    match action {
        AppAction::List => output("app/list", list_apps()),
        AppAction::Launch { bundle, path } => {
            output("app/launch", launch_app(bundle.as_deref(), path.as_deref()))
        }
        AppAction::Inspect {
            pid,
            root,
            limit,
            depth,
        } => output(
            "app/inspect",
            inspect_app(pid, root.as_deref(), limit, depth),
        ),
        AppAction::Press { target } => output("app/press", press_app_element(&target)),
        AppAction::SetValue { target, value } => {
            output("app/set-value", set_app_value(&target, &value))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{Cli, Command};
    use clap::Parser;
    #[test]
    fn parses_app_ax_exploration_and_actions() {
        for args in [
            vec!["app", "inspect", "--pid", "123"],
            vec![
                "app", "inspect", "--pid", "123", "--root", "menu.2", "--depth", "5", "--limit",
                "80",
            ],
            vec!["app", "press", "--target", "observed-token"],
            vec![
                "app",
                "set-value",
                "--target",
                "observed-token",
                "--value",
                "",
            ],
        ] {
            assert!(matches!(
                Cli::try_parse_from(std::iter::once("cueward").chain(args))
                    .expect("app AX arguments")
                    .command,
                Command::App { .. }
            ));
        }
        assert!(Cli::try_parse_from(["cueward", "app", "inspect"]).is_err());
        assert!(Cli::try_parse_from(["cueward", "app", "press"]).is_err());
        assert!(Cli::try_parse_from(["cueward", "app", "set-value", "--target", "token"]).is_err());
    }
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
