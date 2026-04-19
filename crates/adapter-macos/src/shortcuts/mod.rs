use std::process::Command;

use serde::Serialize;

use crate::applescript;
use crate::MacosError;

mod actions;
mod compiler;
mod db;
mod types;

pub use compiler::compile_actions;
pub use db::{
    find_shortcut, find_shortcut_live, list_shortcuts, list_shortcuts_live,
    rename_shortcut_name_by_workflow_id_live, write_shortcut_payload, write_shortcut_payload_live,
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
