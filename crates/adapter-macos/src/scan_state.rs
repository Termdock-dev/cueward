use std::collections::hash_map::Entry;

use chrono::{DateTime, Duration, Utc};
use cueward_core::{ScanTargetState, State};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::MacosError;

pub const NORMAL_MIN_SCAN_INTERVAL: Duration = Duration::minutes(30);
pub const BACKOFF_SCAN_INTERVAL: Duration = Duration::hours(6);
pub const STALE_SKIP_AFTER: Duration = Duration::days(3);
pub const STALE_RECHECK_INTERVAL: Duration = Duration::hours(24);
pub const CONTENT_MAX_AGE_DAYS: i64 = 30;
pub const MIN_CONTENT_WORDS: usize = 5;
pub const MIN_CONTENT_CHARS: usize = 20;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Fresh,
    Unchanged,
    Skipped,
    Warning,
    Deleted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScanEnvelope<T> {
    pub provider: String,
    pub target_url: String,
    pub status: ScanStatus,
    pub no_change_count: u32,
    pub consecutive_not_found_count: u32,
    pub deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checked_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_changed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

pub fn build_scan_key(provider: &str, target_url: &str) -> String {
    format!("{provider}:{target_url}")
}

pub fn load_state() -> State {
    State::load()
}

pub fn stored_entry<'a>(
    state: &'a State,
    key: &str,
) -> Result<&'a ScanTargetState, MacosError> {
    state
        .scan_target(key)
        .ok_or_else(|| MacosError::Other("scan state entry not found".to_string()))
}

pub fn save_state_warning(state: &State) -> Option<String> {
    state
        .save()
        .err()
        .map(|error| format!("failed to save scan state: {error}"))
}

pub fn entry_for<'a>(
    state: &'a mut State,
    key: &str,
    provider: &str,
    target_url: &str,
) -> &'a mut ScanTargetState {
    match state.scan_targets.entry(key.to_string()) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => entry.insert(ScanTargetState {
            provider: provider.to_string(),
            target_url: target_url.to_string(),
            ..ScanTargetState::default()
        }),
    }
}

pub fn skip_reason(entry: &ScanTargetState, now: DateTime<Utc>) -> Option<String> {
    if entry.deleted {
        return Some("deleted_target".to_string());
    }
    let last_checked_at = entry.last_checked_at?;
    if let Some(last_changed_at) = entry.last_changed_at {
        if now - last_changed_at >= STALE_SKIP_AFTER
            && now - last_checked_at < STALE_RECHECK_INTERVAL
        {
            return Some("stale_target_backoff".to_string());
        }
    }
    let min_interval = if entry.no_change_count >= 2 {
        BACKOFF_SCAN_INTERVAL
    } else {
        NORMAL_MIN_SCAN_INTERVAL
    };
    if now - last_checked_at < min_interval {
        return Some("backoff_interval_not_elapsed".to_string());
    }
    None
}

pub fn record_success(
    entry: &mut ScanTargetState,
    fingerprint: String,
    now: DateTime<Utc>,
) -> ScanStatus {
    entry.last_checked_at = Some(now);
    let status = if entry.last_fingerprint.as_deref() == Some(fingerprint.as_str()) {
        entry.no_change_count += 1;
        ScanStatus::Unchanged
    } else {
        entry.last_changed_at = Some(now);
        entry.last_fingerprint = Some(fingerprint);
        entry.no_change_count = 0;
        ScanStatus::Fresh
    };
    entry.consecutive_not_found_count = 0;
    entry.deleted = false;
    status
}

pub fn record_not_found(entry: &mut ScanTargetState, now: DateTime<Utc>) -> ScanStatus {
    entry.last_checked_at = Some(now);
    entry.consecutive_not_found_count += 1;
    if entry.consecutive_not_found_count >= 2 {
        entry.deleted = true;
        ScanStatus::Deleted
    } else {
        ScanStatus::Warning
    }
}

pub fn make_envelope<T>(
    entry: &ScanTargetState,
    status: ScanStatus,
    reason: Option<String>,
    warning: Option<String>,
    data: Option<T>,
) -> ScanEnvelope<T> {
    ScanEnvelope {
        provider: entry.provider.clone(),
        target_url: entry.target_url.clone(),
        status,
        no_change_count: entry.no_change_count,
        consecutive_not_found_count: entry.consecutive_not_found_count,
        deleted: entry.deleted,
        last_checked_at: entry.last_checked_at,
        last_changed_at: entry.last_changed_at,
        reason,
        warning,
        data,
    }
}

pub fn fingerprint_json<T: Serialize>(value: &T) -> Result<String, MacosError> {
    let json = serde_json::to_vec(value)
        .map_err(|error| MacosError::Other(format!("failed to serialize fingerprint payload: {error}")))?;
    let mut hasher = Sha256::new();
    hasher.update(json);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn is_bot_like_author(author: &str) -> bool {
    let normalized = author.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "automoderator" | "[deleted]" | "[removed]"
    )
}

pub fn is_too_short(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.chars().count() < MIN_CONTENT_CHARS
        && trimmed.split_whitespace().count() < MIN_CONTENT_WORDS
}

pub fn is_too_old(created_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now - created_at > Duration::days(CONTENT_MAX_AGE_DAYS)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use cueward_core::State;
    use serde::Serialize;

    use super::{
        BACKOFF_SCAN_INTERVAL, NORMAL_MIN_SCAN_INTERVAL, STALE_RECHECK_INTERVAL, ScanStatus,
        STALE_SKIP_AFTER,
        build_scan_key, entry_for, fingerprint_json, is_bot_like_author, is_too_old, is_too_short,
        make_envelope, record_not_found, record_success, skip_reason,
    };

    #[derive(Serialize)]
    struct FingerprintPayload<'a> {
        id: &'a str,
        content: &'a str,
    }

    #[test]
    fn build_scan_key_combines_provider_and_target() {
        assert_eq!(
            build_scan_key("reddit", "https://old.reddit.com/r/rust.json?limit=20"),
            "reddit:https://old.reddit.com/r/rust.json?limit=20"
        );
    }

    #[test]
    fn skip_reason_honors_backoff_and_stale_targets() {
        let now = Utc.with_ymd_and_hms(2026, 4, 13, 12, 0, 0).unwrap();
        let mut state = State::default();
        let entry = entry_for(&mut state, "x:https://x.com/home", "x", "https://x.com/home");
        entry.last_checked_at = Some(now - NORMAL_MIN_SCAN_INTERVAL + Duration::minutes(5));
        entry.no_change_count = 0;
        assert_eq!(
            skip_reason(entry, now).as_deref(),
            Some("backoff_interval_not_elapsed")
        );

        entry.last_checked_at = Some(now - BACKOFF_SCAN_INTERVAL + Duration::minutes(5));
        entry.no_change_count = 2;
        assert_eq!(
            skip_reason(entry, now).as_deref(),
            Some("backoff_interval_not_elapsed")
        );

        entry.last_checked_at = Some(now - Duration::hours(12));
        entry.last_changed_at = Some(now - STALE_SKIP_AFTER - Duration::hours(1));
        assert_eq!(
            skip_reason(entry, now).as_deref(),
            Some("stale_target_backoff")
        );

        entry.last_checked_at = Some(now - STALE_RECHECK_INTERVAL - Duration::hours(1));
        assert_eq!(skip_reason(entry, now), None);
    }

    #[test]
    fn record_not_found_requires_two_strikes() {
        let now = Utc.with_ymd_and_hms(2026, 4, 13, 12, 0, 0).unwrap();
        let mut state = State::default();
        let entry = entry_for(
            &mut state,
            "reddit:https://old.reddit.com/r/rust/comments/abc.json?limit=500",
            "reddit",
            "https://old.reddit.com/r/rust/comments/abc.json?limit=500",
        );

        assert_eq!(record_not_found(entry, now), ScanStatus::Warning);
        assert_eq!(entry.consecutive_not_found_count, 1);
        assert!(!entry.deleted);

        assert_eq!(record_not_found(entry, now), ScanStatus::Deleted);
        assert_eq!(entry.consecutive_not_found_count, 2);
        assert!(entry.deleted);
    }

    #[test]
    fn fingerprint_json_is_stable_for_same_payload() {
        let left = fingerprint_json(&FingerprintPayload {
            id: "1",
            content: "same",
        })
        .unwrap();
        let right = fingerprint_json(&FingerprintPayload {
            id: "1",
            content: "same",
        })
        .unwrap();
        let changed = fingerprint_json(&FingerprintPayload {
            id: "1",
            content: "different",
        })
        .unwrap();

        assert_eq!(left, right);
        assert_ne!(left, changed);
    }

    #[test]
    fn record_success_tracks_fresh_and_unchanged() {
        let now = Utc.with_ymd_and_hms(2026, 4, 13, 12, 0, 0).unwrap();
        let mut state = State::default();
        let entry = entry_for(&mut state, "x:https://x.com/home", "x", "https://x.com/home");

        assert_eq!(record_success(entry, "abc".to_string(), now), ScanStatus::Fresh);
        assert_eq!(entry.no_change_count, 0);
        assert_eq!(record_success(entry, "abc".to_string(), now), ScanStatus::Unchanged);
        assert_eq!(entry.no_change_count, 1);
    }

    #[test]
    fn filters_detect_deleted_bot_short_and_old_content() {
        let now = Utc.with_ymd_and_hms(2026, 4, 13, 12, 0, 0).unwrap();
        let old = now - Duration::days(31);

        assert!(is_bot_like_author("AutoModerator"));
        assert!(is_bot_like_author("[deleted]"));
        assert!(is_too_short("too short"));
        assert!(!is_too_short("這是一段足夠長但沒有空白的中文內容用來驗證長度判斷"));
        assert!(is_too_old(old, now));
        assert!(!is_too_old(now, now));
    }

    #[test]
    fn make_envelope_copies_entry_metadata() {
        let now = Utc.with_ymd_and_hms(2026, 4, 13, 12, 0, 0).unwrap();
        let mut state = State::default();
        let entry = entry_for(&mut state, "x:https://x.com/home", "x", "https://x.com/home");
        entry.last_checked_at = Some(now);
        entry.last_changed_at = Some(now);
        entry.no_change_count = 2;

        let envelope = make_envelope(
            entry,
            ScanStatus::Skipped,
            Some("backoff_interval_not_elapsed".to_string()),
            None,
            Some(vec!["placeholder"]),
        );

        assert_eq!(envelope.provider, "x");
        assert_eq!(envelope.no_change_count, 2);
        assert_eq!(envelope.status, ScanStatus::Skipped);
        assert_eq!(envelope.reason.as_deref(), Some("backoff_interval_not_elapsed"));
        assert_eq!(envelope.data, Some(vec!["placeholder"]));
    }
}
