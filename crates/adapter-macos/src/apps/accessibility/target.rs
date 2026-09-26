use crate::MacosError;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AppInstance {
    pub pid: i32,
    pub start_seconds: u64,
    pub start_micros: u64,
    pub executable: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Target {
    pub kind: String,
    pub version: u8,
    pub issued_at: u64,
    pub app: AppInstance,
    pub r#ref: String,
    pub root_fingerprint: String,
    pub context: String,
    pub fingerprint: String,
}

pub(super) fn now() -> Result<u64, MacosError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| MacosError::Other(format!("invalid system clock: {e}")))
}

pub(super) fn ref_depth(reference: &str) -> Result<usize, MacosError> {
    let parts: Vec<_> = reference.split('.').collect();
    let number = |s: &str| {
        !s.is_empty()
            && s.bytes().all(|b| b.is_ascii_digit())
            && (s.len() == 1 || !s.starts_with('0'))
            && s.parse::<isize>().is_ok()
    };
    let valid_root = parts[0] == "menu"
        || parts[0]
            .strip_prefix('w')
            .is_some_and(|s| number(s) && s.parse::<usize>().is_ok_and(|n| n < 64));
    if !valid_root || parts.len() > 13 || !parts.iter().skip(1).all(|s| number(s)) {
        return Err(MacosError::Other(
            "app root must be an observed ref such as w0, w0.1, or menu.0; at most 12 levels deep"
                .into(),
        ));
    }
    Ok(parts.len() - 1)
}

impl Target {
    pub fn encode(&self) -> Result<String, MacosError> {
        serde_json::to_vec(self)
            .map(|data| URL_SAFE_NO_PAD.encode(data))
            .map_err(|e| MacosError::Other(format!("cannot encode app target: {e}")))
    }

    pub fn decode(token: &str, current_time: u64) -> Result<Self, MacosError> {
        let invalid =
            || MacosError::Other("invalid or expired app target; inspect the app again".into());
        if token.len() > 16_384 {
            return Err(invalid());
        }
        let data = URL_SAFE_NO_PAD.decode(token).map_err(|_| invalid())?;
        let target: Self = serde_json::from_slice(&data).map_err(|_| invalid())?;
        if target.kind != "app_ax"
            || target.version != 1
            || target.app.pid <= 0
            || target.app.start_seconds == 0
            || target.app.start_micros >= 1_000_000
            || !std::path::Path::new(&target.app.executable).is_absolute()
            || current_time
                .checked_sub(target.issued_at)
                .is_none_or(|age| age > 300)
            || [
                &target.root_fingerprint,
                &target.context,
                &target.fingerprint,
            ]
            .iter()
            .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(invalid());
        }
        ref_depth(&target.r#ref)?;
        Ok(target)
    }
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
