mod decode;
mod encode;

pub use decode::decompile_actions;
pub use encode::{append_action, compile_actions};

use crate::MacosError;

pub fn compiled_action_count(payload: &[u8]) -> Result<usize, MacosError> {
    plist::from_bytes::<Vec<plist::Value>>(payload)
        .map(|actions| actions.len())
        .map_err(|error| MacosError::Other(format!("failed to count compiled shortcut actions: {error}")))
}

pub(crate) fn inferred_default_output_alias(action_identifier: &str) -> Option<String> {
    default_output_name(action_identifier).map(slugify_alias)
}

pub(crate) fn default_output_name(action_identifier: &str) -> Option<&'static str> {
    match action_identifier {
        "is.workflow.actions.gettext" => Some("Text"),
        "is.workflow.actions.text.replace" => Some("Updated Text"),
        "is.workflow.actions.detect.link" => Some("URLs"),
        "is.workflow.actions.getitemfromlist" => Some("Item from List"),
        "is.workflow.actions.count" => Some("Count"),
        _ => None,
    }
}

fn slugify_alias(input: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }
    out.trim_matches('_').to_string()
}
