use serde_json::{Map, Value};

use cueward_core::{ShortcutAction, ShortcutReference};

use crate::MacosError;

use super::{dedupe_alias, inferred_default_output_alias};

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
            .map(ToOwned::to_owned)
            .or_else(|| inferred_output_alias(params, aliases));

        let decoded_action = match action_identifier {
            "is.workflow.actions.gettext" => decode_get_text_action(params, aliases, output)?,
            "is.workflow.actions.text.replace" => decode_replace_text_action(params, aliases, output)?,
            "is.workflow.actions.detect.link" => decode_get_urls_action(params, aliases, output)?,
            "is.workflow.actions.conditional"
                if params.get("WFControlFlowMode").and_then(Value::as_i64) == Some(0)
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
                from: decode_input_reference(params, aliases, "setclipboard missing WFInput")?,
            },
            "is.workflow.actions.share" => ShortcutAction::Share {
                from: decode_input_reference(params, aliases, "share missing WFInput")?,
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

fn decode_get_text_action(
    params: &Map<String, Value>,
    aliases: &Map<String, Value>,
    output: Option<String>,
) -> Result<ShortcutAction, MacosError> {
    match params.get("WFTextActionText") {
        Some(Value::String(value)) => Ok(ShortcutAction::Text {
            value: value.clone(),
            output,
        }),
        Some(Value::Object(text_token)) => Ok(ShortcutAction::GetText {
            from: decode_attachment_reference(text_token, aliases)?,
            output,
        }),
        _ => Err(MacosError::Other(
            "unsupported gettext payload during export".into(),
        )),
    }
}

fn inferred_output_alias(params: &Map<String, Value>, aliases: &Map<String, Value>) -> Option<String> {
    let uuid = params.get("UUID").and_then(Value::as_str)?;
    aliases.get(uuid).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn decode_get_urls_action(
    params: &Map<String, Value>,
    aliases: &Map<String, Value>,
    output: Option<String>,
) -> Result<ShortcutAction, MacosError> {
    Ok(ShortcutAction::GetUrls {
        from: decode_input_reference(params, aliases, "get-urls missing WFInput")?,
        output,
    })
}

fn decode_replace_text_action(
    params: &Map<String, Value>,
    aliases: &Map<String, Value>,
    output: Option<String>,
) -> Result<ShortcutAction, MacosError> {
    Ok(ShortcutAction::ReplaceText {
        from: decode_input_reference(params, aliases, "replace-text missing WFInput")?,
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
    })
}

fn decode_input_reference(
    params: &Map<String, Value>,
    aliases: &Map<String, Value>,
    missing_message: &str,
) -> Result<ShortcutReference, MacosError> {
    decode_attachment_reference(
        params
            .get("WFInput")
            .and_then(Value::as_object)
            .ok_or_else(|| MacosError::Other(missing_message.into()))?,
        aliases,
    )
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

fn infer_alias(
    action_identifier: &str,
    params: &Map<String, Value>,
    counts: &mut Map<String, Value>,
) -> Option<String> {
    let base = params
        .get("CustomOutputName")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| inferred_default_output_alias(action_identifier))?;

    Some(dedupe_alias(base, counts))
}
