use cueward_core::{
    ShortcutAction, ShortcutInputPolicy, ShortcutReference, ShortcutSpec, ShortcutSurface,
};
use serde_json::json;

use crate::shortcuts::{
    append_action, compile_actions, compiled_action_count, decompile_actions,
};

#[test]
fn compile_actions_builds_text_and_clipboard_chain() {
    let spec = ShortcutSpec {
        name: "Plan Smoke".into(),
        surfaces: vec![],
        input: ShortcutInputPolicy::Any,
        actions: vec![
            ShortcutAction::Text {
                value: "hello".into(),
                output: Some("greeting".into()),
            },
            ShortcutAction::CopyToClipboard {
                from: ShortcutReference::Output("greeting".into()),
            },
        ],
    };

    let payload = compile_actions(&spec).unwrap();
    let actions = plist::from_bytes::<Vec<plist::Value>>(&payload).unwrap();

    assert_eq!(actions.len(), 2);

    let first = actions[0].as_dictionary().unwrap();
    assert_eq!(
        first.get("WFWorkflowActionIdentifier").unwrap().as_string(),
        Some("is.workflow.actions.gettext")
    );
    let first_params = first.get("WFWorkflowActionParameters").unwrap().as_dictionary().unwrap();
    assert_eq!(
        first_params.get("CustomOutputName").and_then(plist::Value::as_string),
        Some("greeting")
    );

    let second = actions[1].as_dictionary().unwrap();
    assert_eq!(
        second.get("WFWorkflowActionIdentifier").unwrap().as_string(),
        Some("is.workflow.actions.setclipboard")
    );
    let second_params = second.get("WFWorkflowActionParameters").unwrap().as_dictionary().unwrap();
    let input = second_params.get("WFInput").unwrap().as_dictionary().unwrap();
    let value = input.get("Value").unwrap().as_dictionary().unwrap();
    assert_eq!(
        value.get("OutputName").and_then(plist::Value::as_string),
        Some("greeting")
    );
}

#[test]
fn append_action_uses_existing_custom_output_name_as_reference() {
    let spec = ShortcutSpec {
        name: "Plan Smoke".into(),
        surfaces: vec![],
        input: ShortcutInputPolicy::Any,
        actions: vec![ShortcutAction::Text {
            value: "hello".into(),
            output: Some("greeting".into()),
        }],
    };

    let existing = compile_actions(&spec).unwrap();
    let appended = append_action(
        &existing,
        &ShortcutAction::CopyToClipboard {
            from: ShortcutReference::Output("greeting".into()),
        },
    )
    .unwrap();

    let actions = plist::from_bytes::<Vec<plist::Value>>(&appended).unwrap();
    assert_eq!(actions.len(), 2);
    let second = actions[1].as_dictionary().unwrap();
    let second_params = second.get("WFWorkflowActionParameters").unwrap().as_dictionary().unwrap();
    let input = second_params.get("WFInput").unwrap().as_dictionary().unwrap();
    let value = input.get("Value").unwrap().as_dictionary().unwrap();
    assert_eq!(
        value.get("OutputName").and_then(plist::Value::as_string),
        Some("greeting")
    );
}

#[test]
fn compile_actions_resolves_input_before_reusing_same_output_name() {
    let spec = ShortcutSpec {
        name: "Alias Reuse".into(),
        surfaces: vec![],
        input: ShortcutInputPolicy::Any,
        actions: vec![
            ShortcutAction::Text {
                value: "hello".into(),
                output: Some("url_text".into()),
            },
            ShortcutAction::GetUrls {
                from: ShortcutReference::Output("url_text".into()),
                output: Some("url_text".into()),
            },
        ],
    };

    let payload = compile_actions(&spec).unwrap();
    let actions = plist::from_bytes::<Vec<plist::Value>>(&payload).unwrap();

    let first_uuid = actions[0]
        .as_dictionary()
        .unwrap()
        .get("WFWorkflowActionParameters")
        .unwrap()
        .as_dictionary()
        .unwrap()
        .get("UUID")
        .and_then(plist::Value::as_string)
        .unwrap()
        .to_string();

    let second_input_uuid = actions[1]
        .as_dictionary()
        .unwrap()
        .get("WFWorkflowActionParameters")
        .unwrap()
        .as_dictionary()
        .unwrap()
        .get("WFInput")
        .unwrap()
        .as_dictionary()
        .unwrap()
        .get("Value")
        .unwrap()
        .as_dictionary()
        .unwrap()
        .get("OutputUUID")
        .and_then(plist::Value::as_string)
        .unwrap()
        .to_string();

    assert_eq!(second_input_uuid, first_uuid);
}

#[test]
fn compile_actions_supports_extension_input_as_direct_reference() {
    let spec = ShortcutSpec {
        name: "Extension Input".into(),
        surfaces: vec![],
        input: ShortcutInputPolicy::Url,
        actions: vec![ShortcutAction::GetUrls {
            from: ShortcutReference::ExtensionInput,
            output: Some("urls".into()),
        }],
    };

    let payload = compile_actions(&spec).unwrap();
    let actions = plist::from_bytes::<Vec<plist::Value>>(&payload).unwrap();
    let params = actions[0]
        .as_dictionary()
        .unwrap()
        .get("WFWorkflowActionParameters")
        .unwrap()
        .as_dictionary()
        .unwrap();
    let input = params.get("WFInput").unwrap().as_dictionary().unwrap();
    let value = input.get("Value").unwrap().as_dictionary().unwrap();

    assert_eq!(
        input.get("WFSerializationType").and_then(plist::Value::as_string),
        Some("WFTextTokenAttachment")
    );
    assert_eq!(value.get("Type").and_then(plist::Value::as_string), Some("ExtensionInput"));
}

#[test]
fn decompile_actions_round_trips_supported_clean_url_subset() {
    let spec = ShortcutSpec {
        name: "Clean URL Share".into(),
        surfaces: vec![ShortcutSurface::ShareSheet, ShortcutSurface::LibraryRoot],
        input: ShortcutInputPolicy::Url,
        actions: vec![
            ShortcutAction::GetText {
                from: ShortcutReference::ExtensionInput,
                output: Some("input_url_text".into()),
            },
            ShortcutAction::GetUrls {
                from: ShortcutReference::Output("input_url_text".into()),
                output: Some("urls".into()),
            },
            ShortcutAction::GetText {
                from: ShortcutReference::Output("urls".into()),
                output: Some("normalized_url_text".into()),
            },
            ShortcutAction::ReplaceText {
                from: ShortcutReference::Output("normalized_url_text".into()),
                find: "foo".into(),
                replace: "bar".into(),
                regex: true,
                ignore_case: true,
                output: Some("tracking_removed".into()),
            },
            ShortcutAction::CopyToClipboard {
                from: ShortcutReference::Output("tracking_removed".into()),
            },
            ShortcutAction::Share {
                from: ShortcutReference::Output("tracking_removed".into()),
            },
        ],
    };

    let payload = compile_actions(&spec).unwrap();
    let decompiled = decompile_actions(&payload).unwrap();

    assert_eq!(decompiled, spec.actions);
}

#[test]
fn decompile_actions_preserves_inferred_default_output_aliases() {
    let actions = vec![
        json!({
            "WFWorkflowActionIdentifier": "is.workflow.actions.gettext",
            "WFWorkflowActionParameters": {
                "UUID": "12345678-1234-1234-1234-1234567890AB",
                "WFTextActionText": "hello"
            }
        }),
        json!({
            "WFWorkflowActionIdentifier": "is.workflow.actions.setclipboard",
            "WFWorkflowActionParameters": {
                "WFInput": {
                    "Value": {
                        "OutputName": "Text",
                        "OutputUUID": "12345678-1234-1234-1234-1234567890AB",
                        "Type": "ActionOutput"
                    },
                    "WFSerializationType": "WFTextTokenAttachment"
                }
            }
        }),
    ];
    let mut payload = Vec::new();
    plist::to_writer_binary(&mut payload, &actions).unwrap();

    let decompiled = decompile_actions(&payload).unwrap();

    assert_eq!(
        decompiled,
        vec![
            ShortcutAction::Text {
                value: "hello".into(),
                output: Some("text".into()),
            },
            ShortcutAction::CopyToClipboard {
                from: ShortcutReference::Output("text".into()),
            },
        ]
    );
}

#[test]
fn compile_and_decompile_if_and_repeat_round_trip() {
    let spec = ShortcutSpec {
        name: "Control Flow Smoke".into(),
        surfaces: vec![],
        input: ShortcutInputPolicy::Any,
        actions: vec![
            ShortcutAction::Text {
                value: "match".into(),
                output: Some("input_text".into()),
            },
            ShortcutAction::IfEqualsText {
                input: ShortcutReference::Output("input_text".into()),
                value: "match".into(),
                then_actions: vec![
                    ShortcutAction::Text {
                        value: "ok".into(),
                        output: Some("condition_result".into()),
                    },
                    ShortcutAction::CopyToClipboard {
                        from: ShortcutReference::Output("condition_result".into()),
                    },
                ],
            },
            ShortcutAction::RepeatEach {
                input: ShortcutReference::Output("input_text".into()),
                body: vec![ShortcutAction::Share {
                    from: ShortcutReference::RepeatItem,
                }],
            },
        ],
    };

    let payload = compile_actions(&spec).unwrap();
    let decompiled = decompile_actions(&payload).unwrap();

    assert_eq!(decompiled, spec.actions);
}

#[test]
fn compiled_action_count_flattens_control_flow_wrappers() {
    let spec = ShortcutSpec {
        name: "Control Flow Smoke".into(),
        surfaces: vec![],
        input: ShortcutInputPolicy::Any,
        actions: vec![
            ShortcutAction::Text {
                value: "match".into(),
                output: Some("input_text".into()),
            },
            ShortcutAction::IfEqualsText {
                input: ShortcutReference::Output("input_text".into()),
                value: "match".into(),
                then_actions: vec![
                    ShortcutAction::Text {
                        value: "ok".into(),
                        output: Some("condition_result".into()),
                    },
                    ShortcutAction::CopyToClipboard {
                        from: ShortcutReference::Output("condition_result".into()),
                    },
                ],
            },
            ShortcutAction::RepeatEach {
                input: ShortcutReference::Output("input_text".into()),
                body: vec![ShortcutAction::Share {
                    from: ShortcutReference::RepeatItem,
                }],
            },
        ],
    };

    let payload = compile_actions(&spec).unwrap();

    assert_eq!(compiled_action_count(&payload).unwrap(), 8);
}
