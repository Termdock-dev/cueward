use clap::Subcommand;
use std::process;

use super::helpers::{print_external, validate_optional_output_path};

#[derive(Subcommand)]
pub(crate) enum ScreenshotAction {
    /// List capturable windows
    Windows,
    /// Capture a specific window
    Window {
        /// Window id from `screenshot windows`
        #[arg(long)]
        id: u32,
        /// Also run OCR on the captured image
        #[arg(long)]
        ocr: bool,
        /// Output path
        #[arg(long)]
        output: Option<String>,
    },
}

pub(crate) fn dispatch(
    ocr: bool,
    output: Option<String>,
    display: Option<u32>,
    action: Option<ScreenshotAction>,
) {
    if let Err(err) = validate_optional_output_path("--output", output.as_deref()) {
        eprintln!("{err}");
        process::exit(1);
    }

    match action {
        None => match cueward_adapter_macos::screenshot::capture(ocr, output.as_deref(), display) {
            Ok(result) => {
                print_external("screenshot", &serde_json::to_string_pretty(&result).unwrap());
                eprintln!("screenshot saved to {}", result.path);
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        Some(ScreenshotAction::Windows) => match cueward_adapter_macos::screenshot::list_capturable_windows() {
            Ok(windows) => {
                print_external("screenshot/windows", &serde_json::to_string_pretty(&windows).unwrap());
                eprintln!("{} window(s)", windows.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        Some(ScreenshotAction::Window {
            id,
            ocr: window_ocr,
            output: window_output,
        }) => {
            let effective_ocr = ocr || window_ocr;
            let effective_output = window_output.or(output);
            if let Err(err) = validate_optional_output_path("--output", effective_output.as_deref()) {
                eprintln!("{err}");
                process::exit(1);
            }
            match cueward_adapter_macos::screenshot::capture_window(
                effective_ocr,
                effective_output.as_deref(),
                id,
            ) {
                Ok(result) => {
                    print_external("screenshot/window", &serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("screenshot saved to {}", result.path);
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
