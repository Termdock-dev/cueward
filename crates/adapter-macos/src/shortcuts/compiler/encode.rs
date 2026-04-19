use std::io::Cursor;

use serde_json::{Map, Value};
use uuid::Uuid;

use cueward_core::{ShortcutAction, ShortcutReference, ShortcutSpec};

use crate::MacosError;

use super::super::actions::{build_action, resolve_reference, variable_wrapper};
use super::{dedupe_alias, default_output_name, inferred_default_output_alias};

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

fn collect_outputs(actions: &[Value]) -> Map<String, Value> {
    let mut outputs = Map::new();
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
        let Some(output_uuid) = params.get("UUID").and_then(Value::as_str) else {
            continue;
        };
        let output_alias = params
            .get("CustomOutputName")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| inferred_default_output_alias(action_identifier));
        let output_name = params
            .get("CustomOutputName")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| default_output_name(action_identifier).map(ToOwned::to_owned));

        if let (Some(output_alias), Some(output_name)) = (output_alias, output_name) {
            let output_alias = dedupe_alias(output_alias, &mut counts);
            outputs.insert(
                output_alias,
                serde_json::json!({
                    "OutputName": output_name,
                    "OutputUUID": output_uuid,
                }),
            );
        }
    }

    outputs
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

fn compile_if_end(grouping_id: &str) -> Value {
    compile_control_flow_end("is.workflow.actions.conditional", grouping_id)
}

fn compile_repeat_each_end(grouping_id: &str) -> Value {
    compile_control_flow_end("is.workflow.actions.repeat.each", grouping_id)
}

fn compile_control_flow_end(identifier: &str, grouping_id: &str) -> Value {
    serde_json::json!({
        "WFWorkflowActionIdentifier": identifier,
        "WFWorkflowActionParameters": {
            "GroupingIdentifier": grouping_id,
            "WFControlFlowMode": 2
        }
    })
}

fn new_grouping_id() -> String {
    Uuid::new_v4().to_string().to_uppercase()
}
