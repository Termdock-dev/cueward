use std::process;

use super::helpers::print_external;

pub(crate) fn dispatch(path: String) {
    match cueward_adapter_macos::ocr::capture(&path) {
        Ok(cues) => {
            let json = serde_json::to_string_pretty(&cues).unwrap();
            print_external("ocr", &json);
            eprintln!("extracted {} cues", cues.len());
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}
