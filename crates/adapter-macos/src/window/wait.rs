use serde::{Deserialize, Serialize};
use serde_json::json;

use super::bridge::run_ax;
use super::input_target::InputTarget;
use super::target::now_seconds;
use crate::MacosError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WaitCondition {
    ElementExists,
    ElementAbsent,
    ValueEquals,
    Enabled,
    WindowGone,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WaitSelector {
    pub role: String,
    pub name: Option<String>,
    pub identifier: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WaitOptions {
    pub condition: WaitCondition,
    pub selector: Option<WaitSelector>,
    pub value: Option<String>,
    pub timeout_ms: u64,
    pub interval_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WaitStatus {
    Matched,
    TimedOut,
    WindowGone,
    WindowChanged,
    Ambiguous,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WaitResult {
    pub window_id: u32,
    pub condition: WaitCondition,
    pub status: WaitStatus,
    pub polls: u64,
    pub elapsed_ms: u64,
    pub match_count: usize,
    pub complete: bool,
    pub matched_ref: Option<String>,
}

fn validate(options: &WaitOptions) -> Result<(), MacosError> {
    if !(100..=20_000).contains(&options.timeout_ms) || !(50..=1_000).contains(&options.interval_ms)
    {
        return Err(MacosError::Other(
            "wait timeout must be 100..=20000 ms and interval 50..=1000 ms".into(),
        ));
    }
    if options.condition != WaitCondition::WindowGone
        && options
            .selector
            .as_ref()
            .is_none_or(|selector| selector.role.is_empty())
    {
        return Err(MacosError::Other(
            "element conditions require --role".into(),
        ));
    }
    if options.condition == WaitCondition::WindowGone && options.selector.is_some() {
        return Err(MacosError::Other(
            "window-gone must omit element selectors".into(),
        ));
    }
    if let Some(selector) = &options.selector {
        // The helper enforces the 512 Swift-character name limit before observing.
        // Keep this byte bound too; Rust scalar counts differ for combining characters.
        if selector.role.len() > 128
            || selector.name.as_ref().is_some_and(|s| s.len() > 4096)
            || selector.identifier.as_ref().is_some_and(|s| s.len() > 4096)
        {
            return Err(MacosError::Other(
                "invalid or oversized wait selector".into(),
            ));
        }
    }
    if (options.condition == WaitCondition::ValueEquals) != options.value.is_some()
        || options.value.as_ref().is_some_and(|s| s.len() > 65_536)
    {
        return Err(MacosError::Other(
            "value-equals requires a value of at most 65536 bytes; other conditions omit it".into(),
        ));
    }
    Ok(())
}

/// Observe a snapshot-bound window until a condition matches, without replaying any action.
pub fn wait_for_window(token: &str, options: &WaitOptions) -> Result<WaitResult, MacosError> {
    validate(options)?;
    let target = InputTarget::decode(token, now_seconds()?)?;
    let source = format!(
        "{}\n{}",
        include_str!("wait_logic.swift"),
        include_str!("ax_wait.swift")
    );
    run_ax(
        &target.window,
        &source,
        &[],
        &json!({
            "options": options, "caller_pid": std::process::id(),
        }),
    )
}

#[cfg(test)]
#[path = "wait_tests.rs"]
mod tests;
