//! AX exploration rooted in an application, independent of CG window ownership.
use crate::MacosError;
use crate::window::{AX_SUPPORT, AccessibilityNode, ActionStatus};
use serde::{Deserialize, Serialize};
use serde_json::json;

mod target;
pub use target::AppInstance;
use target::{Target, now, ref_depth};

#[derive(Debug, Deserialize, Serialize)]
pub struct AppAccessibilityNode {
    #[serde(flatten)]
    pub node: AccessibilityNode,
    pub receiver_pid: i32,
    #[serde(default)]
    pub unavailable_attributes: Vec<String>,
    #[serde(skip_serializing)]
    root_fingerprint: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AppAccessibilitySnapshot {
    pub app: AppInstance,
    pub root_ref: Option<String>,
    pub nodes: Vec<AppAccessibilityNode>,
    pub truncated: bool,
    #[serde(skip_serializing)]
    context: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AppAXActionResult {
    pub action: String,
    pub pid: i32,
    pub receiver_pid: i32,
    pub r#ref: String,
    pub status: ActionStatus,
    pub frontmost_pid_before: i32,
    pub frontmost_pid_after: i32,
    pub foreground_changed: bool,
}

fn source(body: &str) -> String {
    format!(
        "{AX_SUPPORT}\n{}\n{}\n{body}",
        include_str!("app_common.swift"),
        include_str!("app_roots.swift")
    )
}

/// List an app's AX roots, or explore a subtree at a freshly observed root ref.
pub fn inspect_app(
    pid: i32,
    root: Option<&str>,
    limit: usize,
    depth: usize,
) -> Result<AppAccessibilitySnapshot, MacosError> {
    if pid <= 0 || !(1..=500).contains(&limit) || !(1..=12).contains(&depth) {
        return Err(MacosError::Other(
            "app inspect requires a positive PID, limit 1..=500, and depth 1..=12".into(),
        ));
    }
    let root_depth = root.map(ref_depth).transpose()?.unwrap_or(0);
    let issued_at = now()?;
    let mut result: AppAccessibilitySnapshot = super::run_source(
        json!({
            "action": "inspect", "pid": pid, "root": root,
            "limit": limit, "depth": depth.min(12 - root_depth),
        }),
        &source(include_str!("app_inspect.swift")),
    )?;
    if result.app.pid != pid || result.root_ref.as_deref() != root {
        return Err(MacosError::Other(
            "app observation changed; inspect again".into(),
        ));
    }
    if root.is_some() {
        issue_targets(&mut result, issued_at)?;
    }
    Ok(result)
}

fn issue_targets(result: &mut AppAccessibilitySnapshot, issued_at: u64) -> Result<(), MacosError> {
    for item in &mut result.nodes {
        let node = &mut item.node;
        let leaf =
            !node.r#ref.starts_with("menu") || (node.role == "AXMenuItem" && node.child_count == 0);
        let can_set =
            node.settable_value && matches!(node.role.as_str(), "AXTextField" | "AXTextArea");
        if leaf
            && node.enabled != Some(false)
            && (can_set || node.actions.iter().any(|a| a == "AXPress"))
        {
            node.target = Some(
                Target {
                    kind: "app_ax".into(),
                    version: 1,
                    issued_at,
                    app: result.app.clone(),
                    r#ref: node.r#ref.clone(),
                    root_fingerprint: item.root_fingerprint.clone(),
                    context: result.context.clone(),
                    fingerprint: node.fingerprint.clone(),
                }
                .encode()?,
            );
        }
    }
    Ok(())
}

fn act(token: &str, action: &str, value: Option<&str>) -> Result<AppAXActionResult, MacosError> {
    let target = Target::decode(token, now()?)?;
    super::run_source(
        json!({"action": action, "pid": target.app.pid,
            "target": target, "value": value, "caller_pid": std::process::id(),
            "lock_dir": crate::screenshot::ensure_cache_dir()?,
        }),
        &source(include_str!("app_action.swift")),
    )
}

/// Press an observed app AX element without requesting foreground activation.
pub fn press_app_element(token: &str) -> Result<AppAXActionResult, MacosError> {
    act(token, "press", None)
}

/// Assign text through AX and report whether its value was read back successfully.
pub fn set_app_value(token: &str, value: &str) -> Result<AppAXActionResult, MacosError> {
    if value.len() > 65_536 {
        return Err(MacosError::Other(
            "app text value must be at most 65536 bytes".into(),
        ));
    }
    act(token, "set_value", Some(value))
}

#[cfg(test)]
mod action_tests;
#[cfg(test)]
mod live_tests;
#[cfg(test)]
mod tests;
