use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use super::AccessibilitySurface;
use crate::MacosError;
use crate::screenshot::{CapturableWindow, WindowBounds};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct WindowIdentity {
    pub window_id: u32,
    pub owner_pid: i32,
    pub title: String,
    pub bounds: WindowBounds,
}

impl From<&CapturableWindow> for WindowIdentity {
    fn from(window: &CapturableWindow) -> Self {
        Self {
            window_id: window.window_id,
            owner_pid: window.owner_pid,
            title: window.title.clone(),
            bounds: window.bounds,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Target {
    pub version: u8,
    pub issued_at: u64,
    pub window: WindowIdentity,
    #[serde(default)]
    pub surface: AccessibilitySurface,
    pub r#ref: String,
    pub fingerprint: String,
}

impl Target {
    pub fn encode(&self) -> Result<String, MacosError> {
        serde_json::to_vec(self)
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
            .map_err(|error| MacosError::Other(format!("failed to encode window target: {error}")))
    }

    pub fn decode(token: &str, now: u64) -> Result<Self, MacosError> {
        if token.len() > 16_384 {
            return Err(MacosError::Other("window target is too large".into()));
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(token)
            .map_err(|_| MacosError::Other("invalid window target token".into()))?;
        let target: Self = serde_json::from_slice(&bytes)
            .map_err(|_| MacosError::Other("invalid window target token".into()))?;
        let age = now.checked_sub(target.issued_at);
        let supported = matches!(
            (target.version, target.surface),
            (1, AccessibilitySurface::Window) | (2, AccessibilitySurface::Menu)
        );
        if !supported || age.is_none_or(|age| age > 300) {
            return Err(MacosError::Other(
                "window target expired or unsupported; inspect again".into(),
            ));
        }
        let parts: Vec<_> = target.r#ref.split('.').collect();
        if parts.first() != Some(&"0")
            || parts.len() > 13
            || parts.iter().any(|part| part.parse::<usize>().is_err())
            || target.fingerprint.len() != 64
            || !target
                .fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(MacosError::Other("invalid window element identity".into()));
        }
        Ok(target)
    }
}

pub(super) fn now_seconds() -> Result<u64, MacosError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| MacosError::Other(format!("invalid system clock: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target {
        Target {
            version: 1,
            issued_at: 1_000,
            surface: AccessibilitySurface::Window,
            window: WindowIdentity {
                window_id: 42,
                owner_pid: 123,
                title: "Fixture".into(),
                bounds: WindowBounds {
                    x: 0,
                    y: 0,
                    width: 400,
                    height: 300,
                },
            },
            r#ref: "0.1.2".into(),
            fingerprint: "a".repeat(64),
        }
    }

    #[test]
    fn target_expiry_and_clock_changes_require_a_new_inspection() {
        let token = target().encode().expect("token");
        assert!(Target::decode(&token, 1_000).is_ok());
        assert!(Target::decode(&token, 1_300).is_ok());
        assert!(Target::decode(&token, 1_301).is_err());
        assert!(Target::decode(&token, 999).is_err());
    }

    #[test]
    fn malformed_target_cannot_reach_an_action() {
        assert!(Target::decode("invalid!", 1_000).is_err());
        assert!(Target::decode(&"a".repeat(16_385), 1_000).is_err());
        for path in ["", "1", "0..1", "0.-1", "0.text"] {
            let mut target = target();
            target.r#ref = path.into();
            assert!(Target::decode(&target.encode().expect("token"), 1_000).is_err());
        }
        let mut target = target();
        target.version = 2;
        assert!(Target::decode(&target.encode().expect("token"), 1_000).is_err());
    }

    #[test]
    fn menu_targets_require_their_own_version_and_preserve_legacy_window_tokens() {
        let mut legacy = serde_json::to_value(target()).expect("legacy target");
        legacy.as_object_mut().expect("object").remove("surface");
        let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&legacy).expect("legacy JSON"));
        assert_eq!(
            Target::decode(&encoded, 1000).expect("legacy").surface,
            AccessibilitySurface::Window
        );
        let mut menu = target();
        menu.surface = AccessibilitySurface::Menu;
        assert!(Target::decode(&menu.encode().expect("v1 menu"), 1000).is_err());
        menu.version = 2;
        assert_eq!(
            Target::decode(&menu.encode().expect("menu"), 1000)
                .expect("v2")
                .surface,
            AccessibilitySurface::Menu
        );
        menu.surface = AccessibilitySurface::Window;
        assert!(Target::decode(&menu.encode().expect("v2 window"), 1000).is_err());
    }
}
