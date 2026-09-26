use std::io::Write;
use std::process::Command;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::process::run_with_timeout;
use super::target::WindowIdentity;
use crate::MacosError;

pub(super) fn run_ax<T: DeserializeOwned>(
    window: &WindowIdentity,
    body: &str,
    arguments: &[String],
    request: &Value,
) -> Result<T, MacosError> {
    let source = format!(
        "{}\n{}\n{body}",
        include_str!("ax_elements.swift"),
        include_str!("ax_common.swift")
    );
    let mut script = tempfile::NamedTempFile::with_suffix(".swift")
        .map_err(|error| MacosError::Other(format!("failed to create AX script: {error}")))?;
    script
        .write_all(source.as_bytes())
        .map_err(|error| MacosError::Other(format!("failed to write AX script: {error}")))?;
    let payload = serde_json::to_vec(request)
        .map_err(|error| MacosError::Other(format!("invalid AX request: {error}")))?;
    let mut command = Command::new("swift");
    command
        .arg(script.path())
        .arg(window.owner_pid.to_string())
        .arg(window.window_id.to_string())
        .arg(&window.title)
        .arg(window.bounds.x.to_string())
        .arg(window.bounds.y.to_string())
        .arg(window.bounds.width.to_string())
        .arg(window.bounds.height.to_string())
        .args(arguments);
    let output = run_with_timeout(&mut command, &payload, Duration::from_secs(40))
        .map_err(|error| MacosError::Other(format!("swift AX helper failed: {error}")))?;
    parse_output(output)
}

fn parse_output<T: DeserializeOwned>(output: std::process::Output) -> Result<T, MacosError> {
    if !output.status.success() {
        if output.status.code() == Some(3) {
            return Err(MacosError::Other(
                "Accessibility permission is required; allow the terminal app in System Settings > Privacy & Security > Accessibility. If already enabled after an app update, remove and re-add the terminal app to refresh its permission".into(),
            ));
        }
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!(
            "window accessibility operation failed: {}. Inspect the current state before retrying an action",
            if message.trim().is_empty() {
                "AX helper exited without a result"
            } else {
                message.trim()
            }
        )));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        MacosError::Other(format!(
            "invalid AX result: {error}; inspect before retrying an action"
        ))
    })
}
