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
    /// Ask a selected app to open one local file without requesting activation.
    Open {
        #[arg(long, required_unless_present = "path", conflicts_with = "path")]
        bundle: Option<String>,
        #[arg(long, required_unless_present = "bundle", conflicts_with = "bundle")]
        path: Option<PathBuf>,
        #[arg(long)]
        file: PathBuf,
        /// Request another process; app state and restored windows may still be shared.
        #[arg(long)]
        new_instance: bool,
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
        inspect_app, launch_app, list_apps, open_file, press_app_element, set_app_value,
    };
    match action {
        AppAction::List => output("app/list", list_apps()),
        AppAction::Launch { bundle, path } => {
            output("app/launch", launch_app(bundle.as_deref(), path.as_deref()))
        }
        AppAction::Open {
            bundle,
            path,
            file,
            new_instance,
        } => output(
            "app/open",
            open_file(bundle.as_deref(), path.as_deref(), &file, new_instance),
        ),
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
    fn parses_file_open_with_one_app_and_explicit_new_instance() {
        let command = Cli::try_parse_from([
            "cueward",
            "app",
            "open",
            "--bundle",
            "org.example.Editor",
            "--file",
            "/tmp/Document.txt",
        ])
        .expect("open")
        .command;
        assert!(matches!(
            command,
            Command::App {
                action: AppAction::Open {
                    new_instance: false,
                    ..
                }
            }
        ));
        let command = Cli::try_parse_from([
            "cueward",
            "app",
            "open",
            "--path",
            "/tmp/Editor.app",
            "--file",
            "/tmp/Document.txt",
            "--new-instance",
        ])
        .expect("new instance")
        .command;
        assert!(matches!(
            command,
            Command::App {
                action: AppAction::Open {
                    new_instance: true,
                    ..
                }
            }
        ));
    }

    #[test]
    fn file_open_requires_a_file_and_exactly_one_app_selector() {
        for arguments in [
            vec!["app", "open", "--file", "/tmp/Document.txt"],
            vec!["app", "open", "--bundle", "org.example.Editor"],
            vec![
                "app",
                "open",
                "--file",
                "/tmp/Document.txt",
                "--bundle",
                "org.example.Editor",
                "--path",
                "/tmp/Editor.app",
            ],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("cueward").chain(arguments)).is_err());
        }
    }
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
