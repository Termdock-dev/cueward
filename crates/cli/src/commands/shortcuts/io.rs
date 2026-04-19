use std::fs;

use cueward_core::{ShortcutAction, ShortcutSpec};

pub(super) fn load_shortcut_spec(path: &str) -> Result<ShortcutSpec, String> {
    let source =
        fs::read_to_string(path).map_err(|err| format!("failed to read shortcut spec '{path}': {err}"))?;
    serde_yaml::from_str(&source)
        .map_err(|err| format!("failed to parse shortcut spec '{path}': {err}"))
}

pub(super) fn load_actions_file(path: &str) -> Result<Vec<ShortcutAction>, String> {
    let source = fs::read_to_string(path)
        .map_err(|err| format!("failed to read shortcut actions file '{path}': {err}"))?;
    serde_yaml::from_str(&source)
        .map_err(|err| format!("failed to parse shortcut actions file '{path}': {err}"))
}
