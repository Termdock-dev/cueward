use std::io::Cursor;

use serde_json::{Map, Value};

use cueward_core::{ShortcutAction, ShortcutSpec};

use crate::MacosError;

use super::actions::build_action;

pub fn compile_actions(spec: &ShortcutSpec) -> Result<Vec<u8>, MacosError> {
    let mut outputs = Map::<String, Value>::new();
    let mut actions = Vec::<Value>::with_capacity(spec.actions.len());

    for action in &spec.actions {
        actions.push(build_action(action, &mut outputs)?);
    }

    let value = plist::to_value(&actions)
        .map_err(|error| MacosError::Other(format!("failed to convert shortcut actions to plist value: {error}")))?;

    let mut buffer = Cursor::new(Vec::new());
    plist::to_writer_binary(&mut buffer, &value)
        .map_err(|error| MacosError::Other(format!("failed to encode shortcut actions plist: {error}")))?;
    Ok(buffer.into_inner())
}

fn default_output_name(action_identifier: &str) -> Option<&'static str> {
    match action_identifier {
        "is.workflow.actions.gettext" => Some("Text"),
        "is.workflow.actions.text.replace" => Some("Updated Text"),
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
    actions.push(build_action(action, &mut outputs)?);

    let value = plist::to_value(&actions)
        .map_err(|error| MacosError::Other(format!("failed to convert appended shortcut actions to plist value: {error}")))?;

    let mut buffer = Cursor::new(Vec::new());
    plist::to_writer_binary(&mut buffer, &value)
        .map_err(|error| MacosError::Other(format!("failed to encode appended shortcut actions plist: {error}")))?;
    Ok(buffer.into_inner())
}
