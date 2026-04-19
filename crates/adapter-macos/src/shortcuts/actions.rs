use serde_json::{Map, Value, json};
use uuid::Uuid;

use cueward_core::{ShortcutAction, ShortcutReference};

use crate::MacosError;

fn new_uuid() -> String {
    Uuid::new_v4().to_string().to_uppercase()
}

pub(crate) fn action_output_ref(output_name: &str, output_uuid: &str) -> Value {
    json!({
        "Value": {
            "OutputName": output_name,
            "OutputUUID": output_uuid,
            "Type": "ActionOutput"
        },
        "WFSerializationType": "WFTextTokenAttachment"
    })
}

pub(crate) fn extension_input_text_token() -> Value {
    json!({
        "Value": {
            "attachmentsByRange": {
                "{0, 1}": {
                    "Type": "ExtensionInput"
                }
            },
            "string": "\u{fffc}"
        },
        "WFSerializationType": "WFTextTokenString"
    })
}

fn extension_input_attachment() -> Value {
    json!({
        "Value": {
            "Type": "ExtensionInput"
        },
        "WFSerializationType": "WFTextTokenAttachment"
    })
}

pub(crate) fn text_token_from_output(output_name: &str, output_uuid: &str) -> Value {
    json!({
        "Value": {
            "attachmentsByRange": {
                "{0, 1}": {
                    "OutputName": output_name,
                    "OutputUUID": output_uuid,
                    "Type": "ActionOutput"
                }
            },
            "string": "\u{fffc}"
        },
        "WFSerializationType": "WFTextTokenString"
    })
}

fn variable_ref(variable_name: &str) -> Value {
    json!({
        "Value": {
            "VariableName": variable_name,
            "Type": "Variable"
        },
        "WFSerializationType": "WFTextTokenAttachment"
    })
}

fn text_token_from_variable(variable_name: &str) -> Value {
    json!({
        "Value": {
            "attachmentsByRange": {
                "{0, 1}": {
                    "VariableName": variable_name,
                    "Type": "Variable"
                }
            },
            "string": "\u{fffc}"
        },
        "WFSerializationType": "WFTextTokenString"
    })
}

pub(crate) fn resolve_reference(
    outputs: &Map<String, Value>,
    reference: &ShortcutReference,
    as_text_token: bool,
) -> Result<Value, MacosError> {
    match reference {
        ShortcutReference::Output(name) => {
            let output = outputs
                .get(name)
                .ok_or_else(|| MacosError::Other(format!("unknown shortcut output reference: {name}")))?;
            let output_name = output
                .get("OutputName")
                .and_then(Value::as_str)
                .ok_or_else(|| MacosError::Other(format!("missing output name for reference: {name}")))?;
            let output_uuid = output
                .get("OutputUUID")
                .and_then(Value::as_str)
                .ok_or_else(|| MacosError::Other(format!("missing output uuid for reference: {name}")))?;
            Ok(if as_text_token {
                text_token_from_output(output_name, output_uuid)
            } else {
                action_output_ref(output_name, output_uuid)
            })
        }
        ShortcutReference::ExtensionInput if as_text_token => Ok(extension_input_text_token()),
        ShortcutReference::ExtensionInput => Ok(extension_input_attachment()),
        ShortcutReference::RepeatItem if as_text_token => Ok(text_token_from_variable("Repeat Item")),
        ShortcutReference::RepeatItem => Ok(variable_ref("Repeat Item")),
        ShortcutReference::RepeatIndex if as_text_token => Ok(text_token_from_variable("Repeat Index")),
        ShortcutReference::RepeatIndex => Ok(variable_ref("Repeat Index")),
    }
}

pub(crate) fn variable_wrapper(reference: Value) -> Value {
    json!({
        "Type": "Variable",
        "Variable": reference
    })
}

fn build_text_action(
    value: &str,
    output: Option<&str>,
    outputs: &mut Map<String, Value>,
) -> Value {
    let uuid = new_uuid();
    if let Some(output) = output {
        outputs.insert(
            output.to_string(),
            json!({
                "OutputName": output,
                "OutputUUID": uuid,
            }),
        );
    }
    let mut params = Map::new();
    params.insert("UUID".into(), json!(uuid));
    params.insert("WFTextActionText".into(), json!(value));
    if let Some(output) = output {
        params.insert("CustomOutputName".into(), json!(output));
    }
    json!({
        "WFWorkflowActionIdentifier": "is.workflow.actions.gettext",
        "WFWorkflowActionParameters": params
    })
}

fn build_get_text_action(
    from: &ShortcutReference,
    output: Option<&str>,
    outputs: &mut Map<String, Value>,
) -> Result<Value, MacosError> {
    let uuid = new_uuid();
    if let Some(output) = output {
        outputs.insert(
            output.to_string(),
            json!({
                "OutputName": output,
                "OutputUUID": uuid,
            }),
        );
    }
    let mut params = Map::new();
    params.insert("UUID".into(), json!(uuid));
    params.insert("WFTextActionText".into(), resolve_reference(outputs, from, true)?);
    if let Some(output) = output {
        params.insert("CustomOutputName".into(), json!(output));
    }
    Ok(json!({
        "WFWorkflowActionIdentifier": "is.workflow.actions.gettext",
        "WFWorkflowActionParameters": params
    }))
}

fn build_get_urls_action(
    from: &ShortcutReference,
    output: Option<&str>,
    outputs: &mut Map<String, Value>,
) -> Result<Value, MacosError> {
    let uuid = new_uuid();
    if let Some(output) = output {
        outputs.insert(
            output.to_string(),
            json!({
                "OutputName": output,
                "OutputUUID": uuid,
            }),
        );
    }
    let mut params = Map::new();
    params.insert("UUID".into(), json!(uuid));
    params.insert("WFInput".into(), resolve_reference(outputs, from, false)?);
    if let Some(output) = output {
        params.insert("CustomOutputName".into(), json!(output));
    }
    Ok(json!({
        "WFWorkflowActionIdentifier": "is.workflow.actions.detect.link",
        "WFWorkflowActionParameters": params
    }))
}

fn build_replace_text_action(
    from: &ShortcutReference,
    find: &str,
    replace: &str,
    regex: bool,
    ignore_case: bool,
    output: Option<&str>,
    outputs: &mut Map<String, Value>,
) -> Result<Value, MacosError> {
    let uuid = new_uuid();
    if let Some(output) = output {
        outputs.insert(
            output.to_string(),
            json!({
                "OutputName": output,
                "OutputUUID": uuid,
            }),
        );
    }
    let mut params = Map::new();
    params.insert("UUID".into(), json!(uuid));
    params.insert("WFInput".into(), resolve_reference(outputs, from, true)?);
    params.insert("WFReplaceTextFind".into(), json!(find));
    params.insert("WFReplaceTextReplace".into(), json!(replace));
    params.insert("WFReplaceTextRegularExpression".into(), json!(regex));
    params.insert("WFReplaceTextCaseSensitive".into(), json!(!ignore_case));
    if let Some(output) = output {
        params.insert("CustomOutputName".into(), json!(output));
    }
    Ok(json!({
        "WFWorkflowActionIdentifier": "is.workflow.actions.text.replace",
        "WFWorkflowActionParameters": params
    }))
}

fn build_setclipboard_action(from: &ShortcutReference, outputs: &Map<String, Value>) -> Result<Value, MacosError> {
    Ok(json!({
        "WFWorkflowActionIdentifier": "is.workflow.actions.setclipboard",
        "WFWorkflowActionParameters": {
            "WFInput": resolve_reference(outputs, from, false)?,
        }
    }))
}

fn build_share_action(from: &ShortcutReference, outputs: &Map<String, Value>) -> Result<Value, MacosError> {
    Ok(json!({
        "WFWorkflowActionIdentifier": "is.workflow.actions.share",
        "WFWorkflowActionParameters": {
            "WFInput": resolve_reference(outputs, from, false)?,
        }
    }))
}

pub fn build_action(
    action: &ShortcutAction,
    outputs: &mut Map<String, Value>,
) -> Result<Value, MacosError> {
    match action {
        ShortcutAction::Text { value, output } => Ok(build_text_action(value, output.as_deref(), outputs)),
        ShortcutAction::GetText { from, output } => build_get_text_action(from, output.as_deref(), outputs),
        ShortcutAction::GetUrls { from, output } => build_get_urls_action(from, output.as_deref(), outputs),
        ShortcutAction::ReplaceText {
            from,
            find,
            replace,
            regex,
            ignore_case,
            output,
        } => build_replace_text_action(
            from,
            find,
            replace,
            *regex,
            *ignore_case,
            output.as_deref(),
            outputs,
        ),
        ShortcutAction::CopyToClipboard { from } => build_setclipboard_action(from, outputs),
        ShortcutAction::Share { from } => build_share_action(from, outputs),
        ShortcutAction::IfEqualsText { .. }
        | ShortcutAction::RepeatEach { .. } => Err(MacosError::Other(
            "shortcut action not yet supported by compiler".into(),
        )),
    }
}
