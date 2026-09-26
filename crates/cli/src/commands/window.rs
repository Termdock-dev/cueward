use clap::Subcommand;
use serde::Serialize;
use std::process;

use super::helpers::print_external;

#[derive(Subcommand)]
pub(crate) enum WindowAction {
    /// List titled application windows without activating them.
    List {
        /// Include off-screen windows, such as other Spaces or minimized windows.
        #[arg(long)]
        all_spaces: bool,
    },
    /// Capture a window, including another Space, with image coordinate metadata.
    Snapshot {
        /// Window id from `cueward window list --all-spaces`.
        #[arg(long)]
        id: u32,
        /// Run OCR on the captured window.
        #[arg(long)]
        ocr: bool,
        /// Save the PNG here after the window identity has been rechecked.
        #[arg(long)]
        output: Option<String>,
    },
    /// Inspect one window from `cueward screenshot windows` through Accessibility.
    Inspect {
        /// Window id from `cueward screenshot windows`.
        #[arg(long)]
        id: u32,
        /// Maximum number of accessibility nodes to return.
        #[arg(long, default_value_t = 200)]
        limit: usize,
        /// Maximum depth below the window element.
        #[arg(long, default_value_t = 8)]
        depth: usize,
        /// Capture the same window after reading its accessibility elements.
        #[arg(long)]
        screenshot: bool,
        /// Run OCR on the captured window; implies --screenshot.
        #[arg(long)]
        ocr: bool,
    },
    /// Send AXPress to an inspected element without activating its app.
    Press {
        /// Short-lived target token from `window inspect`.
        #[arg(long)]
        target: String,
    },
    /// Set a text field's AXValue and verify it by reading the value back.
    SetValue {
        /// Short-lived target token from `window inspect`.
        #[arg(long)]
        target: String,
        /// Text to assign to the field.
        #[arg(long, allow_hyphen_values = true)]
        value: String,
    },
}

pub(crate) fn dispatch(action: WindowAction) {
    match action {
        WindowAction::List { all_spaces } => {
            use cueward_adapter_macos::window::{WindowScope, list_windows};
            let scope = if all_spaces {
                WindowScope::AllSpaces
            } else {
                WindowScope::OnScreen
            };
            output("window/list", list_windows(scope));
        }
        WindowAction::Snapshot {
            id,
            ocr,
            output: destination,
        } => output(
            "window/snapshot",
            cueward_adapter_macos::window::snapshot_window(id, ocr, destination.as_deref()),
        ),
        WindowAction::Inspect {
            id,
            limit,
            depth,
            screenshot,
            ocr,
        } => output(
            "window/inspect",
            cueward_adapter_macos::window::inspect_window(id, limit, depth, screenshot, ocr),
        ),
        WindowAction::Press { target } => output(
            "window/press",
            cueward_adapter_macos::window::press(&target),
        ),
        WindowAction::SetValue { target, value } => output(
            "window/set-value",
            cueward_adapter_macos::window::set_value(&target, &value),
        ),
    }
}

fn output<T: Serialize>(source: &str, result: Result<T, cueward_adapter_macos::MacosError>) {
    match result {
        Ok(result) => match serde_json::to_string_pretty(&result) {
            Ok(payload) => print_external(source, &payload),
            Err(error) => {
                eprintln!("error: failed to encode window result: {error}");
                process::exit(1);
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            process::exit(1);
        }
    }
}
