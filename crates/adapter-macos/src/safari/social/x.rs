use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::MacosError;
use crate::scan_state::{
    ScanEnvelope, ScanStatus, build_scan_key, entry_for, fingerprint_json, is_bot_like_author,
    is_too_old, is_too_short, load_state, make_envelope, record_not_found, record_success,
    save_state_warning, skip_reason, stored_entry,
};
use crate::safari_guard::with_safari_session;

use super::super::core::{execute_js_for_profile, focus_tab, open};
use super::super::script::escape_js_string;
use super::SocialFeedPost;

const X_HOME_URL: &str = "https://x.com/home";

#[derive(Serialize)]
struct XPostFingerprint<'a> {
    author: &'a str,
    handle: Option<&'a str>,
    time: Option<&'a str>,
    content: &'a str,
    url: Option<&'a str>,
}

fn x_search_url(query: &str) -> String {
    let encoded = urlencoding::encode(query);
    format!("https://x.com/search?q={encoded}&src=typed_query&f=live")
}

fn x_extract_feed_js() -> String {
    r#"(() => {
        const tweets = document.querySelectorAll('article[data-testid="tweet"]');
        const posts = [];
        const seen = new Set();

        for (const tweet of tweets) {
            const timeEl = tweet.querySelector("time");
            const time = timeEl ? (timeEl.getAttribute("datetime") || "") : "";
            const tweetText = tweet.querySelector('div[data-testid="tweetText"]');
            const content = tweetText ? (tweetText.innerText || "").trim() : "";

            const statusLink = tweet.querySelector("a[href*='/status/']");
            let postUrl = statusLink ? (statusLink.href || statusLink.getAttribute("href") || "") : "";
            if (postUrl.startsWith("/")) {
                postUrl = "https://x.com" + postUrl;
            }

            const key = postUrl || content.substring(0, 50);
            if (seen.has(key) || !content) continue;
            seen.add(key);

            const userCell = tweet.querySelector('div[data-testid="User-Name"]');
            const userText = userCell ? (userCell.innerText || "") : "";
            const author = userText.split("\n")[0] || "";
            const handle = (userText.match(/@\w+/) || [""])[0];

            const groups = tweet.querySelectorAll('div[role="group"] button');
            const metrics = [...groups]
                .map(b => (b.getAttribute("aria-label") || "").trim())
                .filter(Boolean);

            posts.push({
                author,
                handle: handle || null,
                time: time || null,
                content: content.substring(0, 500),
                url: postUrl || null,
                metrics
            });
        }
        return JSON.stringify(posts);
    })()"#
        .to_string()
}

fn poll_x_posts(
    timeout_seconds: u64,
    profile_filter: Option<&str>,
) -> Result<Vec<SocialFeedPost>, MacosError> {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    let js = x_extract_feed_js();

    loop {
        let raw = execute_js_for_profile(&js, profile_filter, "safari_x_feed")?;
        let posts: Vec<SocialFeedPost> = serde_json::from_str(&raw)
            .map_err(|e| MacosError::Other(format!("failed to parse x feed: {e}")))?;

        if !posts.is_empty() || Instant::now() >= deadline {
            return Ok(posts);
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn navigate_tab_or_open(
    url: &str,
    tab_hint: &str,
    profile_filter: Option<&str>,
    context: &str,
) -> Result<(), MacosError> {
    let nav_js = format!(
        r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
        url = escape_js_string(url),
    );
    if focus_tab(tab_hint, profile_filter).is_ok() {
        let _ = execute_js_for_profile(&nav_js, profile_filter, context)?;
    } else {
        let _ = open(url, profile_filter)?;
    }
    Ok(())
}

fn normalize_x_post_url(input: &str) -> Result<String, MacosError> {
    let trimmed = input.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let (host, rest) = without_scheme
        .split_once('/')
        .ok_or_else(|| MacosError::Other(format!("invalid x post url: {input}")))?;
    let host = host.to_ascii_lowercase();
    if !matches!(
        host.as_str(),
        "x.com" | "www.x.com" | "twitter.com" | "www.twitter.com"
    ) {
        return Err(MacosError::Other(format!("invalid x post url: {input}")));
    }
    let path_only = rest
        .split('?')
        .next()
        .unwrap_or(rest)
        .split('#')
        .next()
        .unwrap_or(rest)
        .trim_matches('/');
    if !path_only.contains("/status/") {
        return Err(MacosError::Other(format!("invalid x post url: {input}")));
    }
    Ok(format!("https://x.com/{path_only}"))
}

fn parse_x_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn filter_x_posts(posts: Vec<SocialFeedPost>, now: DateTime<Utc>) -> Vec<SocialFeedPost> {
    posts.into_iter()
        .filter(|post| !is_bot_like_author(&post.author))
        .filter(|post| !is_too_short(&post.content))
        .filter(|post| {
            post.time
                .as_deref()
                .and_then(parse_x_time)
                .map(|created_at| !is_too_old(created_at, now))
                .unwrap_or(true)
        })
        .collect()
}

fn fingerprint_x_posts(posts: &[SocialFeedPost]) -> Result<String, MacosError> {
    let payload: Vec<_> = posts
        .iter()
        .map(|post| XPostFingerprint {
            author: &post.author,
            handle: post.handle.as_deref(),
            time: post.time.as_deref(),
            content: &post.content,
            url: post.url.as_deref(),
        })
        .collect();
    fingerprint_json(&payload)
}

pub fn x_extract_feed(
    profile_filter: Option<&str>,
) -> Result<ScanEnvelope<Vec<SocialFeedPost>>, MacosError> {
    with_safari_session(|| {
        let now = Utc::now();
        let target_url = X_HOME_URL.to_string();
        let key = build_scan_key("x", &target_url);
        let mut state = load_state();
        {
            let entry = entry_for(&mut state, &key, "x", &target_url);
            if entry.deleted {
                return Ok(make_envelope(
                    entry,
                    ScanStatus::Deleted,
                    Some("deleted_target".to_string()),
                    None,
                    None,
                ));
            }
            if let Some(reason) = skip_reason(entry, now) {
                return Ok(make_envelope(entry, ScanStatus::Skipped, Some(reason), None, None));
            }
        }
        let _ = focus_tab("x.com", profile_filter);
        let posts = filter_x_posts(poll_x_posts(5, profile_filter)?, now);
        let fingerprint = fingerprint_x_posts(&posts)?;
        let status = {
            let entry = entry_for(&mut state, &key, "x", &target_url);
            record_success(entry, fingerprint, now)
        };
        let warning = save_state_warning(&state);
        let entry = stored_entry(&state, &key)?;
        Ok(make_envelope(entry, status, None, warning, Some(posts)))
    })
}

pub fn x_search(
    query: &str,
    profile_filter: Option<&str>,
) -> Result<ScanEnvelope<Vec<SocialFeedPost>>, MacosError> {
    with_safari_session(|| {
        let now = Utc::now();
        let target_url = x_search_url(query);
        let key = build_scan_key("x", &target_url);
        let mut state = load_state();
        {
            let entry = entry_for(&mut state, &key, "x", &target_url);
            if entry.deleted {
                return Ok(make_envelope(
                    entry,
                    ScanStatus::Deleted,
                    Some("deleted_target".to_string()),
                    None,
                    None,
                ));
            }
            if let Some(reason) = skip_reason(entry, now) {
                return Ok(make_envelope(entry, ScanStatus::Skipped, Some(reason), None, None));
            }
        }
        navigate_tab_or_open(&target_url, "x.com", profile_filter, "safari_x_search_navigate")?;
        let posts = filter_x_posts(poll_x_posts(10, profile_filter)?, now);
        let fingerprint = fingerprint_x_posts(&posts)?;
        let status = {
            let entry = entry_for(&mut state, &key, "x", &target_url);
            record_success(entry, fingerprint, now)
        };
        let warning = save_state_warning(&state);
        let entry = stored_entry(&state, &key)?;
        Ok(make_envelope(entry, status, None, warning, Some(posts)))
    })
}

pub fn x_read_post(
    url: &str,
    profile_filter: Option<&str>,
) -> Result<ScanEnvelope<Vec<SocialFeedPost>>, MacosError> {
    with_safari_session(|| {
        let now = Utc::now();
        let target_url = normalize_x_post_url(url)?;
        let key = build_scan_key("x", &target_url);
        let mut state = load_state();
        {
            let entry = entry_for(&mut state, &key, "x", &target_url);
            if entry.deleted {
                return Ok(make_envelope(
                    entry,
                    ScanStatus::Deleted,
                    Some("deleted_target".to_string()),
                    None,
                    None,
                ));
            }
            if let Some(reason) = skip_reason(entry, now) {
                return Ok(make_envelope(entry, ScanStatus::Skipped, Some(reason), None, None));
            }
        }
        navigate_tab_or_open(&target_url, "x.com", profile_filter, "safari_x_read_navigate")?;
        let raw_posts = poll_x_posts(10, profile_filter)?;
        if raw_posts.is_empty() {
            let status = {
                let entry = entry_for(&mut state, &key, "x", &target_url);
                record_not_found(entry, now)
            };
            let warning = save_state_warning(&state);
            let entry = stored_entry(&state, &key)?;
            let reason = match status {
                ScanStatus::Deleted => Some("target_confirmed_deleted".to_string()),
                ScanStatus::Warning => Some("target_missing_first_strike".to_string()),
                _ => None,
            };
            return Ok(make_envelope(entry, status, reason, warning, None));
        }
        let posts = filter_x_posts(raw_posts, now);
        let fingerprint = fingerprint_x_posts(&posts)?;
        let status = {
            let entry = entry_for(&mut state, &key, "x", &target_url);
            record_success(entry, fingerprint, now)
        };
        let warning = save_state_warning(&state);
        let entry = stored_entry(&state, &key)?;
        Ok(make_envelope(entry, status, None, warning, Some(posts)))
    })
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::{filter_x_posts, normalize_x_post_url, x_extract_feed_js, x_search_url};
    use super::SocialFeedPost;

    #[test]
    fn x_search_url_encodes_query() {
        let url = x_search_url("台灣 AI 最新討論");

        assert_eq!(
            url,
            "https://x.com/search?q=%E5%8F%B0%E7%81%A3%20AI%20%E6%9C%80%E6%96%B0%E8%A8%8E%E8%AB%96&src=typed_query&f=live"
        );
    }

    #[test]
    fn x_extract_feed_script_includes_status_urls() {
        let script = x_extract_feed_js();

        assert!(script.contains("a[href*='/status/']"));
        assert!(script.contains("url: postUrl || null"));
        assert!(script.contains("https://x.com"));
    }

    #[test]
    fn normalize_x_post_url_strips_query_and_legacy_host() {
        let url = normalize_x_post_url("https://twitter.com/user/status/123?context=3#top")
            .expect("normalized url");

        assert_eq!(url, "https://x.com/user/status/123");
    }

    #[test]
    fn filter_x_posts_drops_deleted_short_and_old_items() {
        let now = Utc::now();
        let posts = vec![
            SocialFeedPost {
                author: "AutoModerator".to_string(),
                handle: None,
                time: Some(now.to_rfc3339()),
                content: "this post is long enough to pass length checks".to_string(),
                url: Some("https://x.com/1".to_string()),
                metrics: Vec::new(),
            },
            SocialFeedPost {
                author: "real_user".to_string(),
                handle: None,
                time: Some((now - Duration::days(31)).to_rfc3339()),
                content: "this post is also long enough to pass the filter".to_string(),
                url: Some("https://x.com/2".to_string()),
                metrics: Vec::new(),
            },
            SocialFeedPost {
                author: "real_user".to_string(),
                handle: None,
                time: Some(now.to_rfc3339()),
                content: "too short".to_string(),
                url: Some("https://x.com/3".to_string()),
                metrics: Vec::new(),
            },
            SocialFeedPost {
                author: "real_user".to_string(),
                handle: None,
                time: Some(now.to_rfc3339()),
                content: "this post is long enough and recent so it should survive".to_string(),
                url: Some("https://x.com/4".to_string()),
                metrics: Vec::new(),
            },
        ];

        let filtered = filter_x_posts(posts, now);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].url.as_deref(), Some("https://x.com/4"));
    }
}
