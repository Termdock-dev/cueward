use std::process::Command;

use serde::Serialize;

use cueward_core::{ShortcutAction, ShortcutReference, ShortcutSpec};

use crate::applescript;
use crate::MacosError;

mod actions;
mod compiler;
mod db;
mod types;

pub use compiler::compile_actions;
pub use db::{
    encode_input_classes, ensure_shortcut_relation_live, find_shortcut, find_shortcut_live,
    list_shortcuts, list_shortcuts_live, rename_shortcut_name_by_workflow_id_live,
    sync_shortcut_surfaces_live, update_shortcut_input_classes_live, write_shortcut_payload,
    write_shortcut_payload_live,
};
pub use types::{ShortcutRecord, ShortcutSelector};

#[cfg(test)]
mod tests;

#[derive(Debug, Serialize)]
pub struct ShortcutCreateResult {
    pub workflow_id: String,
    pub name: String,
}

pub fn create_shortcut(name: &str) -> Result<ShortcutCreateResult, MacosError> {
    let output = Command::new("sh")
        .arg("-c")
        .arg("osascript -e 'tell application \"Shortcuts\"' -e 'return id of (make new shortcut)' -e 'end tell'")
        .output()
        .map_err(|err| MacosError::Other(format!("create shortcut shell command failed: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("create shortcut: {stderr}")));
    }

    let workflow_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

    rename_shortcut_name_by_workflow_id_live(&workflow_id, name)?;

    Ok(ShortcutCreateResult {
        workflow_id,
        name: name.to_string(),
    })
}

pub fn run_shortcut(selector: &ShortcutSelector) -> Result<(), MacosError> {
    let record = find_shortcut_live(selector)?;
    let output = Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "Shortcuts Events""#)
        .arg("-e")
        .arg(format!(r#"run shortcut named "{}""#, applescript::escape(&record.name)))
        .arg("-e")
        .arg("end tell")
        .output()
        .map_err(|err| MacosError::Other(format!("run shortcut shell command failed: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("run shortcut: {stderr}")));
    }

    Ok(())
}

fn spec_uses_shortcut_input_variables(spec: &ShortcutSpec) -> bool {
    fn reference_uses_extension_input(reference: &ShortcutReference) -> bool {
        matches!(reference, ShortcutReference::ExtensionInput)
    }

    fn action_uses_extension_input(action: &ShortcutAction) -> bool {
        match action {
            ShortcutAction::Text { .. } => false,
            ShortcutAction::GetText { from, .. }
            | ShortcutAction::GetUrls { from, .. }
            | ShortcutAction::CopyToClipboard { from }
            | ShortcutAction::Share { from } => reference_uses_extension_input(from),
            ShortcutAction::ReplaceText { from, .. } => reference_uses_extension_input(from),
            ShortcutAction::IfEqualsText {
                input,
                then_actions,
                ..
            } => reference_uses_extension_input(input)
                || then_actions.iter().any(action_uses_extension_input),
            ShortcutAction::RepeatEach { input, body } => {
                reference_uses_extension_input(input) || body.iter().any(action_uses_extension_input)
            }
        }
    }

    spec.actions.iter().any(action_uses_extension_input)
}

pub fn apply_shortcut_spec(spec: &ShortcutSpec) -> Result<ShortcutRecord, MacosError> {
    let record = find_shortcut_live(&ShortcutSelector::Name(spec.name.clone()))?;
    let payload = compile_actions(spec)?;
    let input_classes = encode_input_classes(&spec.input)?;
    let has_shortcut_input_variables = spec_uses_shortcut_input_variables(spec);

    write_shortcut_payload_live(
        record.pk,
        &payload,
        spec.actions.len(),
        Some(&input_classes),
        has_shortcut_input_variables,
    )?;
    sync_shortcut_surfaces_live(record.pk, &spec.surfaces)?;
    find_shortcut_live(&ShortcutSelector::Id(record.workflow_id))
}

pub fn rename_shortcut(selector: &ShortcutSelector, new_name: &str) -> Result<ShortcutRecord, MacosError> {
    let record = find_shortcut_live(selector)?;
    rename_shortcut_name_by_workflow_id_live(&record.workflow_id, new_name)?;
    find_shortcut_live(&ShortcutSelector::Id(record.workflow_id))
}

pub fn attach_surface(
    selector: &ShortcutSelector,
    surface: &cueward_core::ShortcutSurface,
) -> Result<ShortcutRecord, MacosError> {
    let record = find_shortcut_live(selector)?;
    match surface {
        cueward_core::ShortcutSurface::LibraryRoot => ensure_shortcut_relation_live(record.pk, 6)?,
        cueward_core::ShortcutSurface::ShareSheet => ensure_shortcut_relation_live(record.pk, 2)?,
        cueward_core::ShortcutSurface::Folder(folder_name) => {
            sync_shortcut_surfaces_live(record.pk, &[cueward_core::ShortcutSurface::Folder(folder_name.clone())])?;
        }
    }
    find_shortcut_live(&ShortcutSelector::Id(record.workflow_id))
}

pub fn set_input_type(
    selector: &ShortcutSelector,
    policy: &cueward_core::ShortcutInputPolicy,
) -> Result<ShortcutRecord, MacosError> {
    let record = find_shortcut_live(selector)?;
    let input_classes = encode_input_classes(policy)?;
    update_shortcut_input_classes_live(record.pk, &input_classes)?;
    find_shortcut_live(&ShortcutSelector::Id(record.workflow_id))
}
