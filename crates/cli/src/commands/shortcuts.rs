use std::{fs, process};

use clap::{Args, Subcommand, ValueEnum};

use cueward_core::{ShortcutAction, ShortcutInputPolicy, ShortcutReference, ShortcutSpec, ShortcutSurface};

use super::helpers::print_external;

#[derive(Debug, Clone, Args)]
pub(crate) struct ShortcutSelectorArgs {
    /// Match by workflow id
    #[arg(long, required_unless_present = "name", conflicts_with = "name")]
    pub(crate) id: Option<String>,
    /// Match by shortcut name
    #[arg(long, required_unless_present = "id", conflicts_with = "id")]
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum ShortcutSurfaceArg {
    ShareSheet,
    LibraryRoot,
}

#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum ShortcutInputTypeArg {
    Any,
    Url,
    Urls,
    Text,
    Image,
    File,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ShortcutsAction {
    /// Create a blank shortcut shell
    Create {
        /// Shortcut name
        name: String,
    },
    /// Show a shortcut as a high-level spec
    Show {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
    },
    /// List shortcuts
    List,
    /// Run a shortcut
    Run {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
    },
    /// Rename a shortcut
    Rename {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        /// New shortcut name
        new_name: String,
    },
    /// Move a shortcut into a folder
    Move {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        /// Folder name
        folder: String,
    },
    /// Attach or detach shortcut surfaces
    Surface {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        /// Surface to add
        surface: ShortcutSurfaceArg,
    },
    /// Set accepted input type
    InputType {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        /// Input type
        input_type: ShortcutInputTypeArg,
    },
    /// Append a text action
    AddText {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        #[arg(long)]
        value: String,
        #[arg(long)]
        output: Option<String>,
    },
    /// Append a get-text action
    AddGetText {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        #[arg(long)]
        from: String,
        #[arg(long)]
        output: Option<String>,
    },
    /// Append a get-urls action
    AddGetUrls {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        #[arg(long)]
        from: String,
        #[arg(long)]
        output: Option<String>,
    },
    /// Append a replace-text action
    AddReplaceText {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        #[arg(long)]
        from: String,
        #[arg(long)]
        find: String,
        #[arg(long)]
        replace: String,
        #[arg(long)]
        regex: bool,
        #[arg(long)]
        ignore_case: bool,
        #[arg(long)]
        output: Option<String>,
    },
    /// Append a copy-to-clipboard action
    AddCopyToClipboard {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        #[arg(long)]
        from: String,
    },
    /// Append a share action
    AddShare {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        #[arg(long)]
        from: String,
    },
    /// Append an if action
    AddIf {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        #[arg(long)]
        input: String,
        #[arg(long)]
        value: String,
    },
    /// Append a repeat action
    AddRepeat {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        #[arg(long)]
        input: String,
    },
    /// Apply a shortcut spec file
    Apply {
        /// Path to YAML spec file
        path: String,
    },
    /// Export a shortcut to a YAML spec
    ExportSpec {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
    },
    /// Validate a YAML shortcut spec
    ValidateSpec {
        /// Path to YAML spec file
        path: String,
    },
}

pub(crate) fn dispatch(_action: ShortcutsAction) {
    match _action {
        ShortcutsAction::Create { name } => match cueward_adapter_macos::shortcuts::create_shortcut(&name) {
            Ok(result) => {
                print_external(
                    "shortcuts/create",
                    &serde_json::to_string_pretty(&result).unwrap(),
                );
                eprintln!("shortcut created: {}", result.workflow_id);
            }
            Err(err) => {
                eprintln!("error: {err}");
                process::exit(1);
            }
        },
        ShortcutsAction::List => match cueward_adapter_macos::shortcuts::list_shortcuts_live() {
            Ok(shortcuts) => {
                print_external(
                    "shortcuts/list",
                    &serde_json::to_string_pretty(&shortcuts).unwrap(),
                );
                eprintln!("{} shortcut(s)", shortcuts.len());
            }
            Err(err) => {
                eprintln!("error: {err}");
                process::exit(1);
            }
        },
        ShortcutsAction::Show { selector } => {
            let selector = selector.into_selector();
            match cueward_adapter_macos::shortcuts::find_shortcut_live(&selector) {
                Ok(shortcut) => {
                    print_external(
                        "shortcuts/show",
                        &serde_json::to_string_pretty(&shortcut).unwrap(),
                    );
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    process::exit(1);
                }
            }
        }
        ShortcutsAction::Run { .. } => {
            eprintln!("error: shortcuts run is not yet implemented");
            process::exit(1);
        }
        ShortcutsAction::Apply { path } => {
            let spec = match load_shortcut_spec(&path) {
                Ok(spec) => spec,
                Err(err) => {
                    eprintln!("error: {err}");
                    process::exit(1);
                }
            };

            match cueward_adapter_macos::shortcuts::apply_shortcut_spec(&spec) {
                Ok(shortcut) => {
                    print_external(
                        "shortcuts/apply",
                        &serde_json::to_string_pretty(&shortcut).unwrap(),
                    );
                    eprintln!("shortcut updated: {}", shortcut.workflow_id);
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    process::exit(1);
                }
            }
        }
        ShortcutsAction::ValidateSpec { path } => {
            let spec = match load_shortcut_spec(&path) {
                Ok(spec) => spec,
                Err(err) => {
                    eprintln!("error: {err}");
                    process::exit(1);
                }
            };

            match cueward_adapter_macos::shortcuts::compile_actions(&spec) {
                Ok(_) => eprintln!("shortcut spec is valid"),
                Err(err) => {
                    eprintln!("error: {err}");
                    process::exit(1);
                }
            }
        }
        ShortcutsAction::ExportSpec { selector } => {
            let selector = selector.into_selector();
            match cueward_adapter_macos::shortcuts::export_shortcut_spec(&selector) {
                Ok(spec) => {
                    let yaml = serde_yaml::to_string(&spec).unwrap();
                    print_external("shortcuts/export-spec", &yaml);
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    process::exit(1);
                }
            }
        }
        ShortcutsAction::Rename { selector, new_name } => {
            let selector = selector.into_selector();
            match cueward_adapter_macos::shortcuts::rename_shortcut(&selector, &new_name) {
                Ok(shortcut) => {
                    print_external(
                        "shortcuts/rename",
                        &serde_json::to_string_pretty(&shortcut).unwrap(),
                    );
                    eprintln!("shortcut renamed: {}", shortcut.workflow_id);
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    process::exit(1);
                }
            }
        }
        ShortcutsAction::Move { selector, folder } => {
            let selector = selector.into_selector();
            match cueward_adapter_macos::shortcuts::move_shortcut(&selector, &folder) {
                Ok(shortcut) => {
                    print_external(
                        "shortcuts/move",
                        &serde_json::to_string_pretty(&shortcut).unwrap(),
                    );
                    eprintln!("shortcut moved: {}", shortcut.workflow_id);
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    process::exit(1);
                }
            }
        }
        ShortcutsAction::Surface { selector, surface } => {
            let selector = selector.into_selector();
            match cueward_adapter_macos::shortcuts::attach_surface(&selector, &surface.into()) {
                Ok(shortcut) => {
                    print_external(
                        "shortcuts/surface",
                        &serde_json::to_string_pretty(&shortcut).unwrap(),
                    );
                    eprintln!("shortcut surface updated: {}", shortcut.workflow_id);
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    process::exit(1);
                }
            }
        }
        ShortcutsAction::InputType { selector, input_type } => {
            let selector = selector.into_selector();
            match cueward_adapter_macos::shortcuts::set_input_type(&selector, &input_type.into()) {
                Ok(shortcut) => {
                    print_external(
                        "shortcuts/input-type",
                        &serde_json::to_string_pretty(&shortcut).unwrap(),
                    );
                    eprintln!("shortcut input type updated: {}", shortcut.workflow_id);
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    process::exit(1);
                }
            }
        }
        ShortcutsAction::AddText { selector, value, output } => {
            let selector = selector.into_selector();
            let action = ShortcutAction::Text { value, output };
            append_action(selector, action, "shortcuts/add-text");
        }
        ShortcutsAction::AddGetText { selector, from, output } => {
            let selector = selector.into_selector();
            let action = ShortcutAction::GetText {
                from: parse_reference(&from),
                output,
            };
            append_action(selector, action, "shortcuts/add-get-text");
        }
        ShortcutsAction::AddGetUrls { selector, from, output } => {
            let selector = selector.into_selector();
            let action = ShortcutAction::GetUrls {
                from: parse_reference(&from),
                output,
            };
            append_action(selector, action, "shortcuts/add-get-urls");
        }
        ShortcutsAction::AddReplaceText {
            selector,
            from,
            find,
            replace,
            regex,
            ignore_case,
            output,
        } => {
            let selector = selector.into_selector();
            let action = ShortcutAction::ReplaceText {
                from: parse_reference(&from),
                find,
                replace,
                regex,
                ignore_case,
                output,
            };
            append_action(selector, action, "shortcuts/add-replace-text");
        }
        ShortcutsAction::AddCopyToClipboard { selector, from } => {
            let selector = selector.into_selector();
            let action = ShortcutAction::CopyToClipboard {
                from: parse_reference(&from),
            };
            append_action(selector, action, "shortcuts/add-copy-to-clipboard");
        }
        ShortcutsAction::AddShare { selector, from } => {
            let selector = selector.into_selector();
            let action = ShortcutAction::Share {
                from: parse_reference(&from),
            };
            append_action(selector, action, "shortcuts/add-share");
        }
        _ => {
            eprintln!("error: shortcuts subcommand not yet implemented");
            process::exit(1);
        }
    }
}

impl ShortcutSelectorArgs {
    fn into_selector(self) -> cueward_adapter_macos::shortcuts::ShortcutSelector {
        match (self.id, self.name) {
            (Some(id), None) => cueward_adapter_macos::shortcuts::ShortcutSelector::Id(id),
            (None, Some(name)) => cueward_adapter_macos::shortcuts::ShortcutSelector::Name(name),
            _ => unreachable!("clap enforces exactly one selector"),
        }
    }
}

fn load_shortcut_spec(path: &str) -> Result<ShortcutSpec, String> {
    let source =
        fs::read_to_string(path).map_err(|err| format!("failed to read shortcut spec '{path}': {err}"))?;
    serde_yaml::from_str(&source)
        .map_err(|err| format!("failed to parse shortcut spec '{path}': {err}"))
}

impl From<ShortcutSurfaceArg> for ShortcutSurface {
    fn from(value: ShortcutSurfaceArg) -> Self {
        match value {
            ShortcutSurfaceArg::ShareSheet => ShortcutSurface::ShareSheet,
            ShortcutSurfaceArg::LibraryRoot => ShortcutSurface::LibraryRoot,
        }
    }
}

impl From<ShortcutInputTypeArg> for ShortcutInputPolicy {
    fn from(value: ShortcutInputTypeArg) -> Self {
        match value {
            ShortcutInputTypeArg::Any => ShortcutInputPolicy::Any,
            ShortcutInputTypeArg::Url => ShortcutInputPolicy::Url,
            ShortcutInputTypeArg::Urls => ShortcutInputPolicy::Urls,
            ShortcutInputTypeArg::Text => ShortcutInputPolicy::Text,
            ShortcutInputTypeArg::Image => ShortcutInputPolicy::Image,
            ShortcutInputTypeArg::File => ShortcutInputPolicy::File,
        }
    }
}

fn parse_reference(input: &str) -> ShortcutReference {
    match input {
        "extension-input" => ShortcutReference::ExtensionInput,
        "repeat-item" => ShortcutReference::RepeatItem,
        "repeat-index" => ShortcutReference::RepeatIndex,
        other => ShortcutReference::Output(other.to_string()),
    }
}

fn append_action(
    selector: cueward_adapter_macos::shortcuts::ShortcutSelector,
    action: ShortcutAction,
    source: &str,
) {
    match cueward_adapter_macos::shortcuts::append_shortcut_action(&selector, &action) {
        Ok(shortcut) => {
            print_external(source, &serde_json::to_string_pretty(&shortcut).unwrap());
            eprintln!("shortcut updated: {}", shortcut.workflow_id);
        }
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(1);
        }
    }
}
