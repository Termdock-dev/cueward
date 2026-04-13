use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScanTargetState {
    pub provider: String,
    pub target_url: String,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_changed_at: Option<DateTime<Utc>>,
    pub last_fingerprint: Option<String>,
    #[serde(default)]
    pub no_change_count: u32,
    #[serde(default)]
    pub consecutive_not_found_count: u32,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    /// Last successful capture timestamp per source
    #[serde(default)]
    pub watermarks: HashMap<String, DateTime<Utc>>,
    #[serde(default)]
    pub scan_targets: HashMap<String, ScanTargetState>,
}

impl State {
    fn path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".cueward/state.json")
    }

    pub fn load() -> Self {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)
    }

    pub fn watermark(&self, source: &str) -> Option<DateTime<Utc>> {
        self.watermarks.get(source).copied()
    }

    pub fn set_watermark(&mut self, source: &str, ts: DateTime<Utc>) {
        let entry = self.watermarks.entry(source.to_owned()).or_insert(ts);
        if ts > *entry {
            *entry = ts;
        }
    }

    pub fn scan_target(&self, key: &str) -> Option<&ScanTargetState> {
        self.scan_targets.get(key)
    }

    pub fn scan_target_mut(&mut self, key: &str) -> Option<&mut ScanTargetState> {
        self.scan_targets.get_mut(key)
    }

    pub fn set_scan_target(&mut self, key: String, state: ScanTargetState) {
        self.scan_targets.insert(key, state);
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{ScanTargetState, State};

    #[test]
    fn load_legacy_state_without_scan_targets() {
        let state: State = serde_json::from_str(r#"{"watermarks":{}}"#).expect("legacy state");

        assert!(state.scan_targets.is_empty());
    }

    #[test]
    fn state_round_trips_scan_targets() {
        let mut state = State::default();
        let now = Utc::now();
        state.set_scan_target(
            "reddit:https://old.reddit.com/r/rust.json?limit=20".to_string(),
            ScanTargetState {
                provider: "reddit".to_string(),
                target_url: "https://old.reddit.com/r/rust.json?limit=20".to_string(),
                last_checked_at: Some(now),
                last_changed_at: Some(now),
                last_fingerprint: Some("abc123".to_string()),
                no_change_count: 2,
                consecutive_not_found_count: 1,
                deleted: false,
            },
        );

        let json = serde_json::to_string(&state).expect("encode state");
        let decoded: State = serde_json::from_str(&json).expect("decode state");

        let entry = decoded
            .scan_target("reddit:https://old.reddit.com/r/rust.json?limit=20")
            .expect("scan target");
        assert_eq!(entry.provider, "reddit");
        assert_eq!(entry.last_fingerprint.as_deref(), Some("abc123"));
        assert_eq!(entry.no_change_count, 2);
        assert_eq!(entry.consecutive_not_found_count, 1);
    }

    #[test]
    fn set_scan_target_replaces_existing_entry() {
        let mut state = State::default();
        let key = "x:https://x.com/home".to_string();

        state.set_scan_target(
            key.clone(),
            ScanTargetState {
                provider: "x".to_string(),
                target_url: "https://x.com/home".to_string(),
                last_checked_at: None,
                last_changed_at: None,
                last_fingerprint: Some("old".to_string()),
                no_change_count: 1,
                consecutive_not_found_count: 0,
                deleted: false,
            },
        );

        state.set_scan_target(
            key.clone(),
            ScanTargetState {
                provider: "x".to_string(),
                target_url: "https://x.com/home".to_string(),
                last_checked_at: None,
                last_changed_at: None,
                last_fingerprint: Some("new".to_string()),
                no_change_count: 0,
                consecutive_not_found_count: 2,
                deleted: true,
            },
        );

        let entry = state.scan_target(&key).expect("updated scan target");
        assert_eq!(entry.last_fingerprint.as_deref(), Some("new"));
        assert_eq!(entry.consecutive_not_found_count, 2);
        assert!(entry.deleted);
    }
}
