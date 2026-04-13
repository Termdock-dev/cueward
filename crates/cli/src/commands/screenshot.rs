use std::process;

use super::helpers::{print_external, validate_optional_output_path};

pub(crate) fn dispatch(ocr: bool, output: Option<String>, display: Option<u32>) {
    if let Err(err) = validate_optional_output_path("--output", output.as_deref()) {
        eprintln!("{err}");
        process::exit(1);
    }
    match cueward_adapter_macos::screenshot::capture(ocr, output.as_deref(), display) {
        Ok(result) => {
            print_external("screenshot", &serde_json::to_string_pretty(&result).unwrap());
            eprintln!("screenshot saved to {}", result.path);
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}
