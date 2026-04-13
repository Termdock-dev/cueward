use chrono::{TimeZone, Utc};
use serde::Serialize;

use crate::scan_state::{
    ScanEnvelope, ScanStatus, build_scan_key, entry_for, fingerprint_json, is_bot_like_author,
    is_too_old, is_too_short, load_state, make_envelope, record_not_found, record_success,
    save_state_warning, skip_reason, stored_entry,
};
use crate::MacosError;

use super::{
    FetchJsonError, RedditComment, RedditFeedResult, RedditPostResult, RedditPostSummary,
    RedditSearchResult, build_about_url, build_feed_url, build_search_url, fetch_json,
    normalize_post_url, normalize_subreddit, parse_listing_posts, parse_post_result,
    parse_subreddit_info, validate_limit,
};

#[derive(Serialize)]
struct RedditPostFingerprint<'a> {
    id: &'a str,
    title: &'a str,
    author: &'a str,
    subreddit: &'a str,
    url: &'a str,
    permalink: &'a str,
    created_utc: i64,
    selftext: Option<&'a str>,
}

#[derive(Serialize)]
struct RedditCommentFingerprint<'a> {
    id: &'a str,
    author: &'a str,
    body: &'a str,
    permalink: &'a str,
    created_utc: i64,
}

fn fingerprint_posts(posts: &[RedditPostSummary]) -> Result<String, MacosError> {
    let payload: Vec<_> = posts
        .iter()
        .map(|post| RedditPostFingerprint {
            id: &post.id,
            title: &post.title,
            author: &post.author,
            subreddit: &post.subreddit,
            url: &post.url,
            permalink: &post.permalink,
            created_utc: post.created_utc,
            selftext: post.selftext.as_deref(),
        })
        .collect();
    fingerprint_json(&payload)
}

fn fingerprint_comments(comments: &[RedditComment]) -> Result<String, MacosError> {
    let payload: Vec<_> = comments
        .iter()
        .map(|comment| RedditCommentFingerprint {
            id: &comment.id,
            author: &comment.author,
            body: &comment.body,
            permalink: &comment.permalink,
            created_utc: comment.created_utc,
        })
        .collect();
    fingerprint_json(&payload)
}

pub(super) fn filter_comments(
    comments: Vec<RedditComment>,
    now: chrono::DateTime<Utc>,
) -> Vec<RedditComment> {
    comments
        .into_iter()
        .filter(|comment| !is_bot_like_author(&comment.author))
        .filter(|comment| !is_too_short(&comment.body))
        .filter(|comment| {
            Utc.timestamp_opt(comment.created_utc, 0)
                .single()
                .map(|created_at| !is_too_old(created_at, now))
                .unwrap_or(true)
        })
        .collect()
}

/// Read a subreddit feed via old.reddit.com JSON endpoints with scan-state tracking.
pub fn feed(subreddit: &str, limit: usize) -> Result<ScanEnvelope<RedditFeedResult>, MacosError> {
    validate_limit(limit)?;
    let subreddit = normalize_subreddit(subreddit)?;
    let target_url = build_feed_url(&subreddit, limit);
    let key = build_scan_key("reddit", &target_url);
    let now = Utc::now();
    let mut state = load_state();

    {
        let entry = entry_for(&mut state, &key, "reddit", &target_url);
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

    let about = fetch_json(&build_about_url(&subreddit)).map_err(MacosError::from)?;
    let listing = fetch_json(&target_url).map_err(MacosError::from)?;
    let result = RedditFeedResult {
        subreddit: parse_subreddit_info(&about)?,
        posts: parse_listing_posts(&listing)?,
    };
    let fingerprint = fingerprint_posts(&result.posts)?;
    let status = {
        let entry = entry_for(&mut state, &key, "reddit", &target_url);
        record_success(entry, fingerprint, now)
    };
    let warning = save_state_warning(&state);
    let entry = stored_entry(&state, &key)?;
    Ok(make_envelope(entry, status, None, warning, Some(result)))
}

/// Read a Reddit post plus its top-level comments with scan-state tracking.
pub fn post(url: &str) -> Result<ScanEnvelope<RedditPostResult>, MacosError> {
    let url = normalize_post_url(url)?;
    let key = build_scan_key("reddit", &url);
    let now = Utc::now();
    let mut state = load_state();

    {
        let entry = entry_for(&mut state, &key, "reddit", &url);
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

    let payload = match fetch_json(&url) {
        Ok(payload) => payload,
        Err(FetchJsonError::NotFound) => {
            let status = {
                let entry = entry_for(&mut state, &key, "reddit", &url);
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
        Err(error) => return Err(MacosError::from(error)),
    };
    let mut result = parse_post_result(&payload)?;
    result.comments = filter_comments(result.comments, now);
    let fingerprint = fingerprint_comments(&result.comments)?;
    let status = {
        let entry = entry_for(&mut state, &key, "reddit", &url);
        record_success(entry, fingerprint, now)
    };
    let warning = save_state_warning(&state);
    let entry = stored_entry(&state, &key)?;
    Ok(make_envelope(entry, status, None, warning, Some(result)))
}

/// Search Reddit posts via the public JSON API with scan-state tracking.
pub fn search(
    query: &str,
    subreddit: Option<&str>,
    limit: usize,
) -> Result<ScanEnvelope<RedditSearchResult>, MacosError> {
    validate_limit(limit)?;
    let query = query.trim();
    if query.is_empty() {
        return Err(MacosError::Other("reddit query must not be empty".to_string()));
    }
    let subreddit = match subreddit {
        Some(value) => Some(normalize_subreddit(value)?),
        None => None,
    };
    let target_url = build_search_url(query, subreddit.as_deref(), limit);
    let key = build_scan_key("reddit", &target_url);
    let now = Utc::now();
    let mut state = load_state();

    {
        let entry = entry_for(&mut state, &key, "reddit", &target_url);
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

    let posts = parse_listing_posts(&fetch_json(&target_url).map_err(MacosError::from)?)?;
    let result = RedditSearchResult {
        query: query.to_string(),
        subreddit,
        limit,
        posts,
    };
    let fingerprint = fingerprint_posts(&result.posts)?;
    let status = {
        let entry = entry_for(&mut state, &key, "reddit", &target_url);
        record_success(entry, fingerprint, now)
    };
    let warning = save_state_warning(&state);
    let entry = stored_entry(&state, &key)?;
    Ok(make_envelope(entry, status, None, warning, Some(result)))
}
