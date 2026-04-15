use chrono::{DateTime, Utc};
use cueward_core::State;
use serde::Serialize;

use crate::MacosError;
use crate::scan_state::{
    BACKOFF_SCAN_INTERVAL, NORMAL_MIN_SCAN_INTERVAL, STALE_RECHECK_INTERVAL, STALE_SKIP_AFTER,
    ScanEnvelope, ScanStatus, build_scan_key, entry_for, fingerprint_json, is_bot_like_author,
    is_too_old, is_too_short, load_state, make_envelope, record_success, save_state_warning,
    stored_entry,
};
use crate::safari_guard::with_safari_session;

use super::super::core::{execute_js_for_profile, focus_tab};
use super::SocialFeedPost;

const THREADS_FEED_URL: &str = "https://www.threads.com/";

#[derive(Serialize)]
struct ThreadsPostFingerprint<'a> {
    author: &'a str,
    time: Option<&'a str>,
    content: &'a str,
    url: Option<&'a str>,
}

fn parse_threads_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn filter_threads_posts(posts: Vec<SocialFeedPost>, now: DateTime<Utc>) -> Vec<SocialFeedPost> {
    posts.into_iter()
        .filter(|post| !is_bot_like_author(&post.author))
        .filter(|post| !is_too_short(&post.content))
        .filter(|post| {
            post.time
                .as_deref()
                .and_then(parse_threads_time)
                .map(|created_at| !is_too_old(created_at, now))
                .unwrap_or(true)
        })
        .collect()
}

fn fingerprint_threads_posts(posts: &[SocialFeedPost]) -> Result<String, MacosError> {
    let payload: Vec<_> = posts
        .iter()
        .map(|post| ThreadsPostFingerprint {
            author: &post.author,
            time: post.time.as_deref(),
            content: &post.content,
            url: post.url.as_deref(),
        })
        .collect();
    fingerprint_json(&payload)
}

fn threads_backoff_reason(
    entry: &cueward_core::ScanTargetState,
    now: DateTime<Utc>,
) -> Option<String> {
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

fn update_threads_scan_state(
    state: &mut State,
    now: DateTime<Utc>,
    posts: Vec<SocialFeedPost>,
) -> Result<ScanEnvelope<Vec<SocialFeedPost>>, MacosError> {
    let target_url = THREADS_FEED_URL.to_string();
    let key = build_scan_key("threads", &target_url);
    let posts = filter_threads_posts(posts, now);
    let fingerprint = fingerprint_threads_posts(&posts)?;
    let status = {
        let entry = entry_for(state, &key, "threads", &target_url);
        record_success(entry, fingerprint, now)
    };
    let entry = stored_entry(state, &key)?;
    Ok(make_envelope(entry, status, None, None, Some(posts)))
}

pub fn threads_extract_feed(
    profile_filter: Option<&str>,
) -> Result<ScanEnvelope<Vec<SocialFeedPost>>, MacosError> {
    with_safari_session(|| {
        let js = r#"(() => {
        const authorLinks = [...document.querySelectorAll('a[href^="/@"]')];
        const seen = new Set();
        const posts = [];

        for (const link of authorLinks) {
            const href = link.getAttribute("href") || "";
            if (href.split("/").length > 3) continue;

            let container = link;
            for (let i = 0; i < 12; i++) {
                if (!container.parentElement) break;
                container = container.parentElement;
                if ((container.innerText || "").length > 30 && container.querySelector("time")) break;
            }

            const key = container.innerText.substring(0, 80);
            if (seen.has(key)) continue;
            seen.add(key);

            const author = href.replace("/", "");
            const timeEl = container.querySelector("time");
            const time = timeEl ? (timeEl.getAttribute("datetime") || timeEl.innerText || "") : "";
            const contentSpans = [...container.querySelectorAll('span[dir="auto"], div[dir="auto"]')];
            let content = "";
            for (const span of contentSpans) {
                const t = (span.innerText || "").trim();
                if (t.length > 20) { content = t; break; }
            }
            if (content) {
                posts.push({ author, time: time || null, content: content.substring(0, 500), url: null });
            }
        }
        return JSON.stringify(posts);
    })()"#;
        let now = Utc::now();
        let mut state = load_state();
        {
            let target_url = THREADS_FEED_URL.to_string();
            let key = build_scan_key("threads", &target_url);
            let entry = entry_for(&mut state, &key, "threads", &target_url);
            if let Some(reason) = threads_backoff_reason(entry, now) {
                return Ok(make_envelope(
                    entry,
                    ScanStatus::Skipped,
                    Some(reason),
                    None,
                    None,
                ));
            }
        }
        let _ = focus_tab("threads.com", profile_filter);
        let raw = execute_js_for_profile(js, profile_filter, "safari_threads_feed")?;
        let posts: Vec<SocialFeedPost> = serde_json::from_str(&raw)
            .map_err(|e| MacosError::Other(format!("failed to parse threads feed: {e}")))?;
        let mut envelope = update_threads_scan_state(&mut state, now, posts)?;
        envelope.warning = save_state_warning(&state);
        Ok(envelope)
    })
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use cueward_core::State;

    use super::{
        filter_threads_posts, threads_backoff_reason, update_threads_scan_state, SocialFeedPost,
        THREADS_FEED_URL,
    };
    use crate::scan_state::{ScanStatus, entry_for};

    fn post(author: &str, time: Option<&str>, content: &str) -> SocialFeedPost {
        SocialFeedPost {
            author: author.to_string(),
            handle: None,
            time: time.map(str::to_string),
            content: content.to_string(),
            url: None,
            metrics: Vec::new(),
        }
    }

    #[test]
    fn filter_threads_posts_drops_short_bot_and_old_items() {
        let now = Utc.with_ymd_and_hms(2026, 4, 15, 12, 0, 0).unwrap();
        let posts = vec![
            post("alice", Some("2026-04-15T10:00:00Z"), "這是一篇正常的 Threads 內容，長度足夠而且不是機器人"),
            post("[deleted]", Some("2026-04-15T10:00:00Z"), "這篇應該被濾掉，因為作者像 deleted"),
            post("bob", Some("2026-04-15T10:00:00Z"), "太短"),
            post(
                "charlie",
                Some("2026-02-01T10:00:00Z"),
                "這篇雖然夠長，但時間太舊，應該被濾掉",
            ),
        ];

        let filtered = filter_threads_posts(posts, now);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].author, "alice");
    }

    #[test]
    fn update_threads_scan_state_marks_repeated_feed_as_unchanged() {
        let now = Utc.with_ymd_and_hms(2026, 4, 15, 12, 0, 0).unwrap();
        let mut state = State::default();
        let posts = vec![post(
            "alice",
            Some("2026-04-15T10:00:00Z"),
            "這是一篇正常的 Threads 內容，長度足夠而且不是機器人",
        )];

        let first = update_threads_scan_state(&mut state, now, posts.clone()).expect("first envelope");
        let second = update_threads_scan_state(&mut state, now + Duration::minutes(31), posts)
            .expect("second envelope");

        assert_eq!(first.status, ScanStatus::Fresh);
        assert_eq!(second.status, ScanStatus::Unchanged);
        assert_eq!(second.target_url, THREADS_FEED_URL);
    }

    #[test]
    fn update_threads_scan_state_skips_when_backoff_not_elapsed() {
        let now = Utc.with_ymd_and_hms(2026, 4, 15, 12, 0, 0).unwrap();
        let mut state = State::default();
        let key = format!("threads:{THREADS_FEED_URL}");
        let entry = entry_for(&mut state, &key, "threads", THREADS_FEED_URL);
        entry.last_checked_at = Some(now - Duration::minutes(5));

        let envelope = update_threads_scan_state(
            &mut state,
            now,
            vec![post(
                "alice",
                Some("2026-04-15T10:00:00Z"),
                "這是一篇正常的 Threads 內容，長度足夠而且不是機器人",
            )],
        )
        .expect("fresh envelope");

        assert_eq!(envelope.status, ScanStatus::Fresh);
        assert!(envelope.reason.is_none());
        assert!(envelope.data.is_some());
    }

    #[test]
    fn threads_backoff_reason_ignores_deleted_flag() {
        let now = Utc.with_ymd_and_hms(2026, 4, 15, 12, 0, 0).unwrap();
        let mut state = State::default();
        let key = format!("threads:{THREADS_FEED_URL}");
        let entry = entry_for(&mut state, &key, "threads", THREADS_FEED_URL);
        entry.deleted = true;

        assert_eq!(threads_backoff_reason(entry, now), None);
    }
}
