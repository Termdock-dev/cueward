use std::io::Cursor;

use serde_json::{Map, Value};

use cueward_core::ShortcutSpec;

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
