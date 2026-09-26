use clap::Subcommand;
use std::process;

use super::helpers::print_external;

#[derive(Subcommand)]
pub(crate) enum WindowAction {
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
}

pub(crate) fn dispatch(action: WindowAction) {
    match action {
        WindowAction::Inspect {
            id,
            limit,
            depth,
            screenshot,
            ocr,
        } => {
            match cueward_adapter_macos::window::inspect_window(id, limit, depth, screenshot, ocr) {
                Ok(result) => match serde_json::to_string_pretty(&result) {
                    Ok(payload) => print_external("window/inspect", &payload),
                    Err(error) => {
                        eprintln!("error: failed to encode window inspection: {error}");
                        process::exit(1);
                    }
                },
                Err(error) => {
                    eprintln!("error: {error}");
                    process::exit(1);
                }
            }
        }
    }
}
