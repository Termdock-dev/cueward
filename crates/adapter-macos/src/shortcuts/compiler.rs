use std::io::Cursor;

use serde_json::{Map, Value};
use uuid::Uuid;

use cueward_core::{ShortcutAction, ShortcutReference, ShortcutSpec};

use crate::MacosError;

use super::actions::{build_action, resolve_reference, variable_wrapper};

pub fn compile_actions(spec: &ShortcutSpec) -> Result<Vec<u8>, MacosError> {
    let mut outputs = Map::<String, Value>::new();
    let actions = compile_action_sequence(&spec.actions, &mut outputs)?;

    let value = plist::to_value(&actions)
        .map_err(|error| MacosError::Other(format!("failed to convert shortcut actions to plist value: {error}")))?;

    let mut buffer = Cursor::new(Vec::new());
    plist::to_writer_binary(&mut buffer, &value)
        .map_err(|error| MacosError::Other(format!("failed to encode shortcut actions plist: {error}")))?;
    Ok(buffer.into_inner())
}

fn new_grouping_id() -> String {
    Uuid::new_v4().to_string().to_uppercase()
}

fn compile_action_sequence(
    actions: &[ShortcutAction],
    outputs: &mut Map<String, Value>,
) -> Result<Vec<Value>, MacosError> {
    let mut compiled = Vec::new();

    for action in actions {
        match action {
            ShortcutAction::IfEqualsText {
                input,
                value,
                then_actions,
            } => {
                let grouping_id = new_grouping_id();
                compiled.push(compile_if_start(input, value, &grouping_id, outputs)?);
                compiled.extend(compile_action_sequence(then_actions, outputs)?);
                compiled.push(compile_if_end(&grouping_id));
            }
            ShortcutAction::RepeatEach { input, body } => {
                let grouping_id = new_grouping_id();
                compiled.push(compile_repeat_each_start(input, &grouping_id, outputs)?);
                compiled.extend(compile_action_sequence(body, outputs)?);
                compiled.push(compile_repeat_each_end(&grouping_id));
            }
            _ => compiled.push(build_action(action, outputs)?),
        }
    }

    Ok(compiled)
}

fn compile_if_start(
    input: &ShortcutReference,
    value: &str,
    grouping_id: &str,
    outputs: &mut Map<String, Value>,
) -> Result<Value, MacosError> {
    Ok(serde_json::json!({
        "WFWorkflowActionIdentifier": "is.workflow.actions.conditional",
        "WFWorkflowActionParameters": {
            "GroupingIdentifier": grouping_id,
            "WFControlFlowMode": 0,
            "WFCondition": 4,
            "WFInput": variable_wrapper(resolve_reference(outputs, input, false)?),
            "WFConditionalActionString": {
                "Value": {
                    "string": value
                },
                "WFSerializationType": "WFTextTokenString"
            }
        }
    }))
}

fn compile_if_end(grouping_id: &str) -> Value {
    serde_json::json!({
        "WFWorkflowActionIdentifier": "is.workflow.actions.conditional",
        "WFWorkflowActionParameters": {
            "GroupingIdentifier": grouping_id,
            "WFControlFlowMode": 2
        }
    })
}

fn compile_repeat_each_start(
    input: &ShortcutReference,
    grouping_id: &str,
    outputs: &mut Map<String, Value>,
) -> Result<Value, MacosError> {
    Ok(serde_json::json!({
        "WFWorkflowActionIdentifier": "is.workflow.actions.repeat.each",
        "WFWorkflowActionParameters": {
            "GroupingIdentifier": grouping_id,
            "WFControlFlowMode": 0,
            "WFInput": resolve_reference(outputs, input, false)?
        }
    }))
}

fn compile_repeat_each_end(grouping_id: &str) -> Value {
    serde_json::json!({
        "WFWorkflowActionIdentifier": "is.workflow.actions.repeat.each",
        "WFWorkflowActionParameters": {
            "GroupingIdentifier": grouping_id,
            "WFControlFlowMode": 2
        }
    })
}

fn default_output_name(action_identifier: &str) -> Option<&'static str> {
    match action_identifier {
        "is.workflow.actions.gettext" => Some("Text"),
        "is.workflow.actions.text.replace" => Some("Updated Text"),
        "is.workflow.actions.detect.link" => Some("URLs"),
        "is.workflow.actions.getitemfromlist" => Some("Item from List"),
        "is.workflow.actions.count" => Some("Count"),
        _ => None,
    }
}

fn collect_outputs(actions: &[Value]) -> Map<String, Value> {
    let mut outputs = Map::new();

    for action in actions {
        let Some(action_dict) = action.as_object() else {
            continue;
        };
        let Some(action_identifier) = action_dict
            .get("WFWorkflowActionIdentifier")
            .and_then(Value::as_str)
        else {
            continue;
        };
        let Some(params) = action_dict
            .get("WFWorkflowActionParameters")
            .and_then(Value::as_object)
        else {
            continue;
        };
        let Some(output_uuid) = params.get("UUID").and_then(Value::as_str) else {
            continue;
        };
        let output_name = params
            .get("CustomOutputName")
            .and_then(Value::as_str)
            .or_else(|| default_output_name(action_identifier));

        if let Some(output_name) = output_name {
            outputs.insert(
                output_name.to_string(),
                serde_json::json!({
                    "OutputName": output_name,
                    "OutputUUID": output_uuid,
                }),
            );
        }
    }

    outputs
}

pub fn append_action(existing_payload: &[u8], action: &ShortcutAction) -> Result<Vec<u8>, MacosError> {
    let mut actions = plist::from_bytes::<Vec<Value>>(existing_payload)
        .map_err(|error| MacosError::Other(format!("failed to decode existing shortcut actions: {error}")))?;
    let mut outputs = collect_outputs(&actions);
    actions.extend(compile_action_sequence(std::slice::from_ref(action), &mut outputs)?);

    let value = plist::to_value(&actions)
        .map_err(|error| MacosError::Other(format!("failed to convert appended shortcut actions to plist value: {error}")))?;

    let mut buffer = Cursor::new(Vec::new());
    plist::to_writer_binary(&mut buffer, &value)
        .map_err(|error| MacosError::Other(format!("failed to encode appended shortcut actions plist: {error}")))?;
    Ok(buffer.into_inner())
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

fn infer_alias(
    action_identifier: &str,
    params: &Map<String, Value>,
    counts: &mut Map<String, Value>,
) -> Option<String> {
    let base = params
        .get("CustomOutputName")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| default_output_name(action_identifier).map(slugify_alias))?;

    let count = counts
        .get(&base)
        .and_then(Value::as_u64)
        .unwrap_or(0);
    counts.insert(base.clone(), Value::from(count + 1));

    if count == 0 {
        Some(base)
    } else {
        Some(format!("{base}_{}", count + 1))
    }
}

fn alias_map(actions: &[Value]) -> Map<String, Value> {
    let mut aliases = Map::new();
    let mut counts = Map::new();

    for action in actions {
        let Some(action_dict) = action.as_object() else {
            continue;
        };
        let Some(action_identifier) = action_dict
            .get("WFWorkflowActionIdentifier")
            .and_then(Value::as_str)
        else {
            continue;
        };
        let Some(params) = action_dict
            .get("WFWorkflowActionParameters")
            .and_then(Value::as_object)
        else {
            continue;
        };
        let Some(uuid) = params.get("UUID").and_then(Value::as_str) else {
            continue;
        };
        if let Some(alias) = infer_alias(action_identifier, params, &mut counts) {
            aliases.insert(uuid.to_string(), Value::String(alias));
        }
    }

    aliases
}

fn decode_reference_value(
    value: &Map<String, Value>,
    aliases: &Map<String, Value>,
) -> Result<ShortcutReference, MacosError> {
    if let Some(kind) = value.get("Type").and_then(Value::as_str) {
        match kind {
            "ExtensionInput" => return Ok(ShortcutReference::ExtensionInput),
            "ActionOutput" => {
                let uuid = value
                    .get("OutputUUID")
                    .and_then(Value::as_str)
                    .ok_or_else(|| MacosError::Other("missing OutputUUID in shortcut reference".into()))?;
                let alias = aliases
                    .get(uuid)
                    .and_then(Value::as_str)
                    .ok_or_else(|| MacosError::Other(format!("unknown output uuid in shortcut payload: {uuid}")))?;
                return Ok(ShortcutReference::Output(alias.to_string()));
            }
            "Variable" => {
                let name = value
                    .get("VariableName")
                    .and_then(Value::as_str)
                    .ok_or_else(|| MacosError::Other("missing VariableName in shortcut reference".into()))?;
                return match name {
                    "Repeat Item" => Ok(ShortcutReference::RepeatItem),
                    "Repeat Index" => Ok(ShortcutReference::RepeatIndex),
                    other => Err(MacosError::Other(format!(
                        "unsupported variable reference in shortcut payload: {other}"
                    ))),
                };
            }
            other => {
                return Err(MacosError::Other(format!(
                    "unsupported shortcut reference type in payload: {other}"
                )))
            }
        }
    }

    Err(MacosError::Other("shortcut reference missing Type".into()))
}

fn decode_attachment_reference(
    container: &Map<String, Value>,
    aliases: &Map<String, Value>,
) -> Result<ShortcutReference, MacosError> {
    let serialization = container
        .get("WFSerializationType")
        .and_then(Value::as_str)
        .ok_or_else(|| MacosError::Other("shortcut attachment missing WFSerializationType".into()))?;
    let value = container
        .get("Value")
        .and_then(Value::as_object)
        .ok_or_else(|| MacosError::Other("shortcut attachment missing Value".into()))?;

    match serialization {
        "WFTextTokenAttachment" => decode_reference_value(value, aliases),
        "WFTextTokenString" => {
            let string = value
                .get("string")
                .and_then(Value::as_str)
                .ok_or_else(|| MacosError::Other("text token missing string".into()))?;
            if string != "\u{fffc}" {
                return Err(MacosError::Other(
                    "only single-placeholder text tokens are supported during export".into(),
                ));
            }
            let attachments = value
                .get("attachmentsByRange")
                .and_then(Value::as_object)
                .ok_or_else(|| MacosError::Other("text token missing attachmentsByRange".into()))?;
            if attachments.len() != 1 {
                return Err(MacosError::Other(
                    "only single attachment text tokens are supported during export".into(),
                ));
            }
            let attachment = attachments
                .values()
                .next()
                .and_then(Value::as_object)
                .ok_or_else(|| MacosError::Other("invalid attachment payload".into()))?;
            decode_reference_value(attachment, aliases)
        }
        other => Err(MacosError::Other(format!(
            "unsupported shortcut attachment serialization: {other}"
        ))),
    }
}

pub fn decompile_actions(payload: &[u8]) -> Result<Vec<ShortcutAction>, MacosError> {
    let actions = plist::from_bytes::<Vec<Value>>(payload)
        .map_err(|error| MacosError::Other(format!("failed to decode shortcut actions payload: {error}")))?;
    let aliases = alias_map(&actions);
    let (decoded, next_index) = decompile_sequence(&actions, 0, &aliases, None)?;
    if next_index != actions.len() {
        return Err(MacosError::Other("did not consume entire shortcut action payload".into()));
    }
    Ok(decoded)
}

fn decode_conditional_input(
    params: &Map<String, Value>,
    aliases: &Map<String, Value>,
) -> Result<ShortcutReference, MacosError> {
    let variable = params
        .get("WFInput")
        .and_then(Value::as_object)
        .and_then(|value| value.get("Variable"))
        .and_then(Value::as_object)
        .ok_or_else(|| MacosError::Other("conditional missing variable wrapper".into()))?;
    decode_attachment_reference(variable, aliases)
}

fn decode_text_token_literal(value: &Map<String, Value>) -> Result<String, MacosError> {
    let serialization = value
        .get("WFSerializationType")
        .and_then(Value::as_str)
        .ok_or_else(|| MacosError::Other("missing text token serialization".into()))?;
    if serialization != "WFTextTokenString" {
        return Err(MacosError::Other(format!(
            "unsupported text token serialization for conditional literal: {serialization}"
        )));
    }
    let text = value
        .get("Value")
        .and_then(Value::as_object)
        .and_then(|inner| inner.get("string"))
        .and_then(Value::as_str)
        .ok_or_else(|| MacosError::Other("missing conditional string literal".into()))?;
    Ok(text.to_string())
}

fn decompile_sequence(
    actions: &[Value],
    mut index: usize,
    aliases: &Map<String, Value>,
    end_group: Option<(&str, &str)>,
) -> Result<(Vec<ShortcutAction>, usize), MacosError> {
    let mut decoded = Vec::new();

    while index < actions.len() {
        let action_dict = actions[index]
            .as_object()
            .ok_or_else(|| MacosError::Other("invalid shortcut action payload".into()))?;
        let action_identifier = action_dict
            .get("WFWorkflowActionIdentifier")
            .and_then(Value::as_str)
            .ok_or_else(|| MacosError::Other("shortcut action missing identifier".into()))?;
        let params = action_dict
            .get("WFWorkflowActionParameters")
            .and_then(Value::as_object)
            .ok_or_else(|| MacosError::Other("shortcut action missing parameters".into()))?;

        if let Some((expected_identifier, expected_group)) = end_group {
            let control_flow_mode = params.get("WFControlFlowMode").and_then(Value::as_i64);
            let grouping_id = params.get("GroupingIdentifier").and_then(Value::as_str);
            if action_identifier == expected_identifier
                && control_flow_mode == Some(2)
                && grouping_id == Some(expected_group)
            {
                return Ok((decoded, index + 1));
            }
        }

        let output = params
            .get("CustomOutputName")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        let decoded_action = match action_identifier {
            "is.workflow.actions.gettext" => {
                match params.get("WFTextActionText") {
                    Some(Value::String(value)) => ShortcutAction::Text {
                        value: value.clone(),
                        output,
                    },
                    Some(Value::Object(text_token)) => ShortcutAction::GetText {
                        from: decode_attachment_reference(text_token, &aliases)?,
                        output,
                    },
                    _ => {
                        return Err(MacosError::Other(
                            "unsupported gettext payload during export".into(),
                        ))
                    }
                }
            }
            "is.workflow.actions.text.replace" => ShortcutAction::ReplaceText {
                from: decode_attachment_reference(
                    params
                        .get("WFInput")
                        .and_then(Value::as_object)
                        .ok_or_else(|| MacosError::Other("replace-text missing WFInput".into()))?,
                    &aliases,
                )?,
                find: params
                    .get("WFReplaceTextFind")
                    .and_then(Value::as_str)
                    .ok_or_else(|| MacosError::Other("replace-text missing find".into()))?
                    .to_string(),
                replace: params
                    .get("WFReplaceTextReplace")
                    .and_then(Value::as_str)
                    .ok_or_else(|| MacosError::Other("replace-text missing replace".into()))?
                    .to_string(),
                regex: params
                    .get("WFReplaceTextRegularExpression")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                ignore_case: !params
                    .get("WFReplaceTextCaseSensitive")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                output,
            },
            "is.workflow.actions.detect.link" => ShortcutAction::GetUrls {
                from: decode_attachment_reference(
                    params
                        .get("WFInput")
                        .and_then(Value::as_object)
                        .ok_or_else(|| MacosError::Other("get-urls missing WFInput".into()))?,
                    &aliases,
                )?,
                output,
            },
            "is.workflow.actions.conditional" if params.get("WFControlFlowMode").and_then(Value::as_i64) == Some(0)
                && params.get("WFCondition").and_then(Value::as_i64) == Some(4) =>
            {
                let grouping_id = params
                    .get("GroupingIdentifier")
                    .and_then(Value::as_str)
                    .ok_or_else(|| MacosError::Other("if action missing GroupingIdentifier".into()))?
                    .to_string();
                let input = decode_conditional_input(params, aliases)?;
                let value = decode_text_token_literal(
                    params
                        .get("WFConditionalActionString")
                        .and_then(Value::as_object)
                        .ok_or_else(|| MacosError::Other("if action missing WFConditionalActionString".into()))?,
                )?;
                let (then_actions, next_index) = decompile_sequence(
                    actions,
                    index + 1,
                    aliases,
                    Some(("is.workflow.actions.conditional", &grouping_id)),
                )?;
                decoded.push(ShortcutAction::IfEqualsText {
                    input,
                    value,
                    then_actions,
                });
                index = next_index;
                continue;
            }
            "is.workflow.actions.repeat.each" if params.get("WFControlFlowMode").and_then(Value::as_i64) == Some(0) =>
            {
                let grouping_id = params
                    .get("GroupingIdentifier")
                    .and_then(Value::as_str)
                    .ok_or_else(|| MacosError::Other("repeat action missing GroupingIdentifier".into()))?
                    .to_string();
                let input = decode_attachment_reference(
                    params
                        .get("WFInput")
                        .and_then(Value::as_object)
                        .ok_or_else(|| MacosError::Other("repeat action missing WFInput".into()))?,
                    aliases,
                )?;
                let (body, next_index) = decompile_sequence(
                    actions,
                    index + 1,
                    aliases,
                    Some(("is.workflow.actions.repeat.each", &grouping_id)),
                )?;
                decoded.push(ShortcutAction::RepeatEach { input, body });
                index = next_index;
                continue;
            }
            "is.workflow.actions.setclipboard" => ShortcutAction::CopyToClipboard {
                from: decode_attachment_reference(
                    params
                        .get("WFInput")
                        .and_then(Value::as_object)
                        .ok_or_else(|| MacosError::Other("setclipboard missing WFInput".into()))?,
                    &aliases,
                )?,
            },
            "is.workflow.actions.share" => ShortcutAction::Share {
                from: decode_attachment_reference(
                    params
                        .get("WFInput")
                        .and_then(Value::as_object)
                        .ok_or_else(|| MacosError::Other("share missing WFInput".into()))?,
                    &aliases,
                )?,
            },
            other => {
                return Err(MacosError::Other(format!(
                    "unsupported shortcut action during export: {other}"
                )))
            }
        };

        decoded.push(decoded_action);
        index += 1;
    }

    if let Some((expected_identifier, expected_group)) = end_group {
        return Err(MacosError::Other(format!(
            "missing control flow terminator for {expected_identifier} group {expected_group}"
        )));
    }

    Ok((decoded, index))
}
