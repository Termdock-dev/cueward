use clap::{Args, Subcommand, ValueEnum};

use cueward_core::{ShortcutInputPolicy, ShortcutReference, ShortcutSurface};

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
        #[arg(long = "then-actions")]
        then_actions: String,
    },
    /// Append a repeat action
    AddRepeat {
        #[command(flatten)]
        selector: ShortcutSelectorArgs,
        #[arg(long)]
        input: String,
        #[arg(long = "body-actions")]
        body_actions: String,
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

impl ShortcutSelectorArgs {
    pub(super) fn into_selector(self) -> cueward_adapter_macos::shortcuts::ShortcutSelector {
        match (self.id, self.name) {
            (Some(id), None) => cueward_adapter_macos::shortcuts::ShortcutSelector::Id(id),
            (None, Some(name)) => cueward_adapter_macos::shortcuts::ShortcutSelector::Name(name),
            _ => unreachable!("clap enforces exactly one selector"),
        }
    }
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

pub(super) fn parse_reference(input: &str) -> ShortcutReference {
    match input {
        "extension-input" => ShortcutReference::ExtensionInput,
        "repeat-item" => ShortcutReference::RepeatItem,
        "repeat-index" => ShortcutReference::RepeatIndex,
        other => ShortcutReference::Output(other.to_string()),
    }
}
