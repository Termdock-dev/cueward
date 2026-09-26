use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use super::super::input_target::InputTarget;
use super::super::target::WindowIdentity;
use crate::MacosError;

#[derive(Debug, Serialize, Deserialize)]
struct SpaceTarget {
    kind: String,
    version: u8,
    issued_at: u64,
    window: WindowIdentity,
    space_ids: Vec<u64>,
}

pub(super) struct MoveTarget {
    pub issued_at: u64,
    pub window: WindowIdentity,
    pub expected_spaces: Option<Vec<u64>>,
}

fn invalid() -> MacosError {
    MacosError::Other(
        "invalid or expired Space move target; observe with space window again".into(),
    )
}

fn valid_spaces(ids: &[u64]) -> bool {
    !ids.is_empty() && ids.len() <= 64 && ids[0] > 0 && ids.windows(2).all(|pair| pair[0] < pair[1])
}

impl MoveTarget {
    /// Bind a Space-only target to the observed window and source membership.
    pub fn issue(
        window: WindowIdentity,
        mut space_ids: Vec<u64>,
        issued_at: u64,
    ) -> Result<String, MacosError> {
        space_ids.sort_unstable();
        if !valid_spaces(&space_ids) || !valid_window(&window) {
            return Err(invalid());
        }
        let target = SpaceTarget {
            kind: "space_move".into(),
            version: 1,
            issued_at,
            window,
            space_ids,
        };
        serde_json::to_vec(&target)
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
            .map_err(|_| invalid())
    }

    /// Validate a membership target or a legacy snapshot target for movement.
    pub fn decode(token: &str, now: u64) -> Result<Self, MacosError> {
        if token.len() > 16_384 {
            return Err(invalid());
        }
        let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|_| invalid())?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if value["kind"] == "window_input" {
            let target = InputTarget::decode(token, now)?;
            return Ok(Self {
                issued_at: target.issued_at,
                window: target.window,
                expected_spaces: None,
            });
        }
        let target: SpaceTarget = serde_json::from_value(value).map_err(|_| invalid())?;
        if target.kind != "space_move"
            || target.version != 1
            || now
                .checked_sub(target.issued_at)
                .is_none_or(|age| age > 300)
            || !valid_window(&target.window)
            || !valid_spaces(&target.space_ids)
        {
            return Err(invalid());
        }
        Ok(Self {
            issued_at: target.issued_at,
            window: target.window,
            expected_spaces: Some(target.space_ids),
        })
    }
}

fn valid_window(window: &WindowIdentity) -> bool {
    window.window_id > 0
        && window.owner_pid > 0
        && window.bounds.width > 0
        && window.bounds.height > 0
}

#[cfg(test)]
#[path = "space_target_tests.rs"]
mod tests;
