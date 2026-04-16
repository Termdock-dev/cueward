use std::thread;
use std::time::{Duration, Instant};

use crate::MacosError;
use crate::applescript::run_capture as run_applescript_capture;
use crate::safari_guard::safari_automation_state;
#[cfg(test)]
pub(crate) use crate::safari_guard::{
    SAFARI_LOCK_TTL_SECS, SafariAutomationSession, SafariLockFile, acquire_safari_lock,
    read_safari_lock, release_safari_lock,
};

pub mod ai;
mod core;
#[cfg(test)]
mod core_tests;
mod history;
mod script;
#[cfg(test)]
mod script_tests;
mod social;
mod types;

pub use ai::{
    GeminiMode, SafariAiImage, SafariAiImageResult, SafariAiReadyResult, SafariAiResponseResult,
    SafariConversation, SafariDeepResearchResult, chatgpt_list_conversations, chatgpt_save_images,
    ensure_chatgpt_home, ensure_gemini_home, ensure_grok_home, gemini_list_conversations,
    gemini_read_conversation, gemini_save_images, gemini_save_media, grok_list_conversations,
    grok_read_conversation, poll_gemini_deep_research, prepare_gemini_mode,
    send_chatgpt_image_prompt, send_chatgpt_prompt, send_gemini_prompt, send_grok_prompt,
    start_gemini_deep_research,
};
pub use core::{
    active, click, close, close_tabs, exec, fill, focus_tab, open, read, scroll, scroll_and_read,
    source, tabs, wait,
};
pub(crate) use core::doctor_live_probe;
pub use history::capture;
pub use social::{SocialFeedPost, threads_extract_feed, x_extract_feed, x_read_post, x_search};
pub use types::{
    SafariClickResult, SafariCloseResult, SafariEvalResult, SafariFillResult, SafariReadResult,
    SafariScrollReadChunk, SafariScrollReadResult, SafariScrollResult, SafariSourceResult,
    SafariTab, SafariWaitResult,
};

const SAFARI_OPERATION_DELAY: Duration = Duration::from_secs(1);
const SAFARI_429_MAX_RETRIES: usize = 3;
const TAB_SEPARATOR: &str = "---TAB_SEP---";
const FIELD_SEPARATOR: &str = "<<<FIELD_SEP>>>";

fn compute_next_safari_operation(
    now: Instant,
    last_operation_at: Option<Instant>,
) -> (Option<Duration>, Instant) {
    match last_operation_at {
        Some(last) if now < last + SAFARI_OPERATION_DELAY => {
            let next_allowed = last + SAFARI_OPERATION_DELAY;
            (Some(next_allowed - now), next_allowed)
        }
        _ => (None, now),
    }
}

fn throttle_safari_operation() -> Result<(), MacosError> {
    let sleep_for = {
        let state = safari_automation_state();
        let mut guard = state
            .lock()
            .map_err(|_| MacosError::Other("safari automation state poisoned".to_string()))?;
        let now = Instant::now();
        let (delay, next_allowed) = compute_next_safari_operation(now, guard.last_operation_at);
        guard.last_operation_at = Some(next_allowed);
        delay
    };

    if let Some(duration) = sleep_for {
        thread::sleep(duration);
    }
    Ok(())
}

fn is_safari_rate_limited(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        return false;
    }

    let normalized = trimmed.to_ascii_lowercase();
    normalized.contains("too many requests")
        || normalized.contains("http 429")
        || normalized.contains("429 too many requests")
        || normalized.contains("\"status\":429")
        || normalized == "rate limit exceeded"
        || normalized == "rate-limited"
}

fn safari_rate_limit_backoff(attempt: usize) -> Duration {
    Duration::from_secs(30 * (attempt as u64 + 1))
}

fn rate_limit_error(context: &str, detail: &str) -> MacosError {
    MacosError::Other(format!(
        "{context}: Safari automation hit rate limit: {detail}"
    ))
}

fn run_capture(script: &str, context: &str) -> Result<String, MacosError> {
    let mut last_detail = None;

    for attempt in 0..=SAFARI_429_MAX_RETRIES {
        throttle_safari_operation()?;
        match run_applescript_capture(script, context) {
            Ok(stdout) => {
                if !is_safari_rate_limited(&stdout) {
                    return Ok(stdout);
                }
                last_detail = Some(stdout.trim().to_string());
            }
            Err(err) => {
                let detail = err.to_string();
                if !is_safari_rate_limited(&detail) {
                    return Err(err);
                }
                last_detail = Some(detail);
            }
        }

        if attempt == SAFARI_429_MAX_RETRIES {
            break;
        }
        thread::sleep(safari_rate_limit_backoff(attempt));
    }

    Err(rate_limit_error(
        context,
        last_detail
            .as_deref()
            .unwrap_or("unknown rate limit response"),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        SAFARI_LOCK_TTL_SECS, SAFARI_OPERATION_DELAY, SafariAutomationSession, SafariLockFile,
        acquire_safari_lock, compute_next_safari_operation, is_safari_rate_limited,
        read_safari_lock, release_safari_lock, safari_automation_state, safari_rate_limit_backoff,
    };
    use std::fs;
    use std::time::Duration;
    use std::time::Instant;
    use tempfile::tempdir;

    #[test]
    fn safari_rate_limit_detection_matches_expected_signals() {
        assert!(is_safari_rate_limited("HTTP 429 Too Many Requests"));
        assert!(is_safari_rate_limited("rate limit exceeded"));
        assert!(is_safari_rate_limited(
            r#"{"status":429,"error":"Too Many Requests"}"#
        ));
        assert!(!is_safari_rate_limited(
            "this article explains how rate limits work"
        ));
        assert!(!is_safari_rate_limited("all good"));
    }

    #[test]
    fn safari_rate_limit_backoff_is_linear() {
        assert_eq!(safari_rate_limit_backoff(0), Duration::from_secs(30));
        assert_eq!(safari_rate_limit_backoff(1), Duration::from_secs(60));
        assert_eq!(safari_rate_limit_backoff(2), Duration::from_secs(90));
    }

    #[test]
    fn safari_lock_rejects_active_owner() {
        let dir = tempdir().expect("tempdir");
        let lock_path = dir.path().join("lock.json");
        let now = 1_700_000_000;

        let payload = SafariLockFile {
            pid: 42,
            acquired_at: now,
            expires_at: now + SAFARI_LOCK_TTL_SECS,
        };
        fs::write(
            &lock_path,
            serde_json::to_vec(&payload).expect("encode lock payload"),
        )
        .expect("write lock");

        let err = acquire_safari_lock(&lock_path, now, 77).expect_err("active lock should fail");
        assert!(err.to_string().contains("locked by pid 42"));
    }

    #[test]
    fn safari_lock_replaces_stale_owner() {
        let dir = tempdir().expect("tempdir");
        let lock_path = dir.path().join("lock.json");
        let now = 1_700_000_000;

        let stale = SafariLockFile {
            pid: 42,
            acquired_at: now - 600,
            expires_at: now - 1,
        };
        fs::write(
            &lock_path,
            serde_json::to_vec(&stale).expect("encode lock payload"),
        )
        .expect("write stale lock");

        acquire_safari_lock(&lock_path, now, 77).expect("stale lock should be replaced");
        let lock = read_safari_lock(&lock_path).expect("replacement lock");
        assert_eq!(lock.pid, 77);
        assert_eq!(lock.expires_at, now + SAFARI_LOCK_TTL_SECS);
    }

    #[test]
    fn safari_lock_release_removes_owned_lock() {
        let dir = tempdir().expect("tempdir");
        let lock_path = dir.path().join("lock.json");
        let now = 1_700_000_000;

        acquire_safari_lock(&lock_path, now, 77).expect("acquire lock");
        release_safari_lock(&lock_path, 77).expect("release lock");

        assert!(read_safari_lock(&lock_path).is_none());
    }

    #[test]
    fn safari_lock_ttl_covers_long_running_safari_jobs() {
        assert!(SAFARI_LOCK_TTL_SECS >= 900);
    }

    #[test]
    fn safari_lock_reports_corrupted_active_file() {
        let dir = tempdir().expect("tempdir");
        let lock_path = dir.path().join("lock.json");
        let now = 1_700_000_000;

        fs::write(&lock_path, b"{not-json").expect("write corrupt lock");

        let err = acquire_safari_lock(&lock_path, now, 77)
            .expect_err("corrupted active lock should fail");
        assert!(err.to_string().contains("corrupted or unreadable"));
    }

    #[test]
    fn throttle_schedule_reserves_next_available_slot() {
        let now = Instant::now();
        let last = now;

        let (delay, next_allowed) = compute_next_safari_operation(now, Some(last));

        assert_eq!(delay, Some(SAFARI_OPERATION_DELAY));
        assert!(next_allowed >= last + SAFARI_OPERATION_DELAY);
    }

    #[test]
    fn dropping_outer_session_keeps_last_operation_timestamp() {
        let state = safari_automation_state();
        let now = Instant::now();

        {
            let mut guard = state.lock().expect("state lock");
            guard.depth = 1;
            guard.last_operation_at = Some(now);
            guard.lock_path = None;
            guard.lock_owner_pid = None;
        }

        let session = SafariAutomationSession { outermost: true };
        drop(session);

        let guard = state.lock().expect("state lock");
        assert_eq!(guard.depth, 0);
        assert_eq!(guard.last_operation_at, Some(now));
    }
}
