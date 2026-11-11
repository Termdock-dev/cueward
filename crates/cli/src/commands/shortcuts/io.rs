use std::fs;

use cueward_core::{ShortcutAction, ShortcutSpec};

/// Format a shortcut export without reading or writing the Shortcuts app.
pub(super) fn export_spec_envelope(spec: &ShortcutSpec) -> Result<String, String> {
    let yaml = serde_yaml::to_string(spec)
        .map_err(|e| format!("failed to serialize shortcut spec: {e}"))?;
    Ok(crate::commands::helpers::external_envelope(
        "shortcuts/export-spec",
        &yaml,
    ))
}

pub(super) fn load_shortcut_spec(path: &str) -> Result<ShortcutSpec, String> {
    let source = fs::read_to_string(path)
        .map_err(|err| format!("failed to read shortcut spec '{path}': {err}"))?;
    serde_yaml::from_str(&source)
        .map_err(|err| format!("failed to parse shortcut spec '{path}': {err}"))
}

pub(super) fn load_actions_file(path: &str) -> Result<Vec<ShortcutAction>, String> {
    let source = fs::read_to_string(path)
        .map_err(|err| format!("failed to read shortcut actions file '{path}': {err}"))?;
    serde_yaml::from_str(&source)
        .map_err(|err| format!("failed to parse shortcut actions file '{path}': {err}"))
}
