use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShortcutSpec {
    pub name: String,
    #[serde(default)]
    pub surfaces: Vec<ShortcutSurface>,
    pub input: ShortcutInputPolicy,
    #[serde(default)]
    pub actions: Vec<ShortcutAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ShortcutSurface {
    LibraryRoot,
    ShareSheet,
    Folder(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShortcutInputPolicy {
    Any,
    Url,
    Urls,
    Text,
    Image,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShortcutAction {
    Text {
        value: String,
        output: Option<String>,
    },
    GetText {
        from: ShortcutReference,
        output: Option<String>,
    },
    GetUrls {
        from: ShortcutReference,
        output: Option<String>,
    },
    ReplaceText {
        from: ShortcutReference,
        find: String,
        replace: String,
        regex: bool,
        ignore_case: bool,
        output: Option<String>,
    },
    CopyToClipboard {
        from: ShortcutReference,
    },
    Share {
        from: ShortcutReference,
    },
    IfEqualsText {
        input: ShortcutReference,
        value: String,
        then_actions: Vec<ShortcutAction>,
    },
    RepeatEach {
        input: ShortcutReference,
        body: Vec<ShortcutAction>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ShortcutReference {
    Output(String),
    ExtensionInput,
    RepeatItem,
    RepeatIndex,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_round_trip_preserves_clean_url_share_shape() {
        let yaml = r#"
name: Clean URL Share
surfaces:
  - library-root
  - share-sheet
input:
  type: url
actions:
  - type: get-text
    from:
      kind: extension-input
    output: input_url_text
"#;

        let spec: ShortcutSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.name, "Clean URL Share");
        assert_eq!(spec.surfaces.len(), 2);
        assert_eq!(spec.actions.len(), 1);
    }
}
