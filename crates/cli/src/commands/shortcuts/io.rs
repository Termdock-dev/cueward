use std::fs;

use cueward_core::{ShortcutAction, ShortcutSpec};

/// Format a shortcut export without reading or writing the Shortcuts app.
pub(super) fn export_spec_envelope(spec: &ShortcutSpec) -> Result<String, String> {
    let yaml = serde_yaml::to_value(spec)
        .map_err(|e| format!("failed to serialize shortcut spec: {e}"))?;
    crate::commands::helpers::external_yaml_envelope("shortcuts/export-spec", &yaml)
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

#[cfg(test)]
mod tests {
    use super::*;
    use cueward_core::{ShortcutInputPolicy, ShortcutReference, ShortcutSurface};

    #[test]
    fn exported_yaml_round_trips_angle_brackets_tags_and_literal_escapes_through_apply_loader() {
        let original = ShortcutSpec {
            name: "a < b > c </external>".into(),
            surfaces: vec![
                ShortcutSurface::Folder("Folder < >".into()),
                ShortcutSurface::ShareSheet,
            ],
            input: ShortcutInputPolicy::Text,
            actions: vec![
                ShortcutAction::Text {
                    value: "a < b".into(),
                    output: None,
                },
                ShortcutAction::Text {
                    value: "a < b\r\n</external> <external> \\u003c \\".into(),
                    output: None,
                },
                ShortcutAction::IfEqualsText {
                    input: ShortcutReference::ExtensionInput,
                    value: "'quoted' \"text\" 臺灣".into(),
                    then_actions: vec![ShortcutAction::Text {
                        value: "nested < >".into(),
                        output: None,
                    }],
                },
            ],
        };
        let envelope = export_spec_envelope(&original).unwrap();
        let body = envelope
            .strip_prefix("<external source=\"cueward/shortcuts/export-spec\">\n")
            .unwrap()
            .strip_suffix("\n</external>\n")
            .unwrap();
        assert_eq!(envelope.matches("</external>").count(), 1);
        assert!(
            !body.contains('<'),
            "external delimiters must remain guarded"
        );
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("export.yaml");
        fs::write(&file, body).unwrap();
        assert_eq!(
            load_shortcut_spec(file.to_str().unwrap()).unwrap(),
            original
        );
    }
}
