use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::MacosError;

const REDDIT_BASE: &str = "https://old.reddit.com";
const REDDIT_OPERATION_DELAY: Duration = Duration::from_secs(1);
const REDDIT_MAX_RETRIES: usize = 3;
const REDDIT_UA: &str = concat!(
    "cueward/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/HCYT/cueward)"
);

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RedditSubredditInfo {
    pub name: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribers: Option<u64>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RedditPostSummary {
    pub id: String,
    pub title: String,
    pub author: String,
    pub subreddit: String,
    pub url: String,
    pub permalink: String,
    pub score: i64,
    pub num_comments: u64,
    pub created_utc: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selftext: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RedditComment {
    pub id: String,
    pub author: String,
    pub body: String,
    pub score: i64,
    pub created_utc: i64,
    pub permalink: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RedditFeedResult {
    pub subreddit: RedditSubredditInfo,
    pub posts: Vec<RedditPostSummary>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RedditPostResult {
    pub post: RedditPostSummary,
    pub comments: Vec<RedditComment>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RedditSearchResult {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subreddit: Option<String>,
    pub limit: usize,
    pub posts: Vec<RedditPostSummary>,
}

/// Read a subreddit feed via old.reddit.com JSON endpoints.
pub fn feed(subreddit: &str, limit: usize) -> Result<RedditFeedResult, MacosError> {
    validate_limit(limit)?;
    let subreddit = normalize_subreddit(subreddit)?;
    let about = fetch_json(&build_about_url(&subreddit))?;
    let listing = fetch_json(&build_feed_url(&subreddit, limit))?;
    Ok(RedditFeedResult {
        subreddit: parse_subreddit_info(&about)?,
        posts: parse_listing_posts(&listing)?,
    })
}

/// Read a Reddit post plus its top-level comments.
pub fn post(url: &str) -> Result<RedditPostResult, MacosError> {
    let url = normalize_post_url(url)?;
    let payload = fetch_json(&url)?;
    parse_post_result(&payload)
}

/// Search Reddit posts via the public JSON API.
pub fn search(
    query: &str,
    subreddit: Option<&str>,
    limit: usize,
) -> Result<RedditSearchResult, MacosError> {
    validate_limit(limit)?;
    let query = query.trim();
    if query.is_empty() {
        return Err(MacosError::Other("reddit query must not be empty".to_string()));
    }
    let subreddit = match subreddit {
        Some(value) => Some(normalize_subreddit(value)?),
        None => None,
    };
    let posts = parse_listing_posts(&fetch_json(&build_search_url(query, subreddit.as_deref(), limit))?)?;
    Ok(RedditSearchResult {
        query: query.to_string(),
        subreddit,
        limit,
        posts,
    })
}

fn validate_limit(limit: usize) -> Result<(), MacosError> {
    if limit == 0 {
        return Err(MacosError::Other("reddit limit must be greater than 0".to_string()));
    }
    Ok(())
}

fn normalize_subreddit(input: &str) -> Result<String, MacosError> {
    let trimmed = input.trim();
    let trimmed = trimmed.strip_prefix("r/").unwrap_or(trimmed);
    let trimmed = trimmed.strip_prefix("R/").unwrap_or(trimmed);
    let trimmed = trimmed.trim_matches('/');
    if trimmed.is_empty() || trimmed.contains('/') {
        return Err(MacosError::Other(format!("invalid subreddit: {input}")));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn normalize_post_url(input: &str) -> Result<String, MacosError> {
    let trimmed = input.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let (host, rest) = without_scheme
        .split_once('/')
        .ok_or_else(|| MacosError::Other(format!("invalid reddit post url: {input}")))?;
    let host = host.to_ascii_lowercase();
    if !matches!(host.as_str(), "reddit.com" | "www.reddit.com" | "old.reddit.com") {
        return Err(MacosError::Other(format!("invalid reddit post url: {input}")));
    }
    let path_only = rest
        .split('?')
        .next()
        .unwrap_or(rest)
        .split('#')
        .next()
        .unwrap_or(rest);
    let path = path_only.trim_matches('/');
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() < 4 || segments[0] != "r" || segments[2] != "comments" {
        return Err(MacosError::Other(format!("invalid reddit post url: {input}")));
    }
    let subreddit = normalize_subreddit(segments[1])?;
    let post_id = segments[3];
    if post_id.is_empty() {
        return Err(MacosError::Other(format!("invalid reddit post url: {input}")));
    }
    let slug = segments.get(4).copied().unwrap_or("");
    let suffix = if slug.is_empty() {
        String::new()
    } else {
        format!("/{slug}")
    };
    Ok(format!(
        "{REDDIT_BASE}/r/{subreddit}/comments/{post_id}{suffix}.json?limit=500"
    ))
}

fn build_about_url(subreddit: &str) -> String {
    format!("{REDDIT_BASE}/r/{subreddit}/about.json")
}

fn build_feed_url(subreddit: &str, limit: usize) -> String {
    format!("{REDDIT_BASE}/r/{subreddit}.json?limit={limit}")
}

fn build_search_url(query: &str, subreddit: Option<&str>, limit: usize) -> String {
    let query = urlencoding::encode(query);
    match subreddit {
        Some(subreddit) => format!(
            "{REDDIT_BASE}/r/{subreddit}/search.json?q={query}&restrict_sr=on&limit={limit}&sort=relevance"
        ),
        None => format!("{REDDIT_BASE}/search.json?q={query}&limit={limit}&sort=relevance"),
    }
}

fn fetch_json(url: &str) -> Result<Value, MacosError> {
    let mut last_error = None;

    for attempt in 0..=REDDIT_MAX_RETRIES {
        throttle_reddit_request()?;
        let response = ureq::get(url)
            .set("User-Agent", REDDIT_UA)
            .set("Accept", "application/json")
            .call();
        match response {
            Ok(response) => {
                let body = response.into_string().map_err(|error| {
                    MacosError::Other(format!("failed to read reddit response: {error}"))
                })?;
                return serde_json::from_str(&body).map_err(|error| {
                    MacosError::Other(format!("invalid reddit json payload: {error}"))
                });
            }
            Err(ureq::Error::Status(status, response)) => {
                let retry_after = retry_after_delay(&response);
                let body = response.into_string().unwrap_or_default();
                if should_retry_status(status) && attempt < REDDIT_MAX_RETRIES {
                    last_error = Some(format!(
                        "reddit returned retryable status {status}: {}",
                        body.trim()
                    ));
                    thread::sleep(retry_after.unwrap_or_else(|| reddit_backoff(attempt)));
                    continue;
                }
                return Err(MacosError::Other(format!(
                    "reddit returned unexpected status {status}: {}",
                    body.trim()
                )));
            }
            Err(ureq::Error::Transport(error)) => {
                if attempt < REDDIT_MAX_RETRIES {
                    last_error = Some(format!("reddit request failed: {error}"));
                    thread::sleep(reddit_backoff(attempt));
                    continue;
                }
                return Err(MacosError::Other(format!("reddit request failed: {error}")));
            }
        }
    }

    Err(MacosError::Other(
        last_error.unwrap_or_else(|| "reddit request failed after retries".to_string()),
    ))
}

fn reddit_request_state() -> &'static Mutex<Option<Instant>> {
    static STATE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

fn compute_next_reddit_request(
    now: Instant,
    last_request_at: Option<Instant>,
) -> (Option<Duration>, Instant) {
    match last_request_at {
        Some(last) if now < last + REDDIT_OPERATION_DELAY => {
            let next_allowed = last + REDDIT_OPERATION_DELAY;
            (Some(next_allowed - now), next_allowed)
        }
        _ => (None, now),
    }
}

fn throttle_reddit_request() -> Result<(), MacosError> {
    let sleep_for = {
        let mut guard = reddit_request_state()
            .lock()
            .map_err(|_| MacosError::Other("reddit request state poisoned".to_string()))?;
        let now = Instant::now();
        let (delay, next_allowed) = compute_next_reddit_request(now, *guard);
        *guard = Some(next_allowed);
        delay
    };

    if let Some(delay) = sleep_for {
        thread::sleep(delay);
    }
    Ok(())
}

fn should_retry_status(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}

fn retry_after_delay(response: &ureq::Response) -> Option<Duration> {
    response
        .header("Retry-After")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

fn reddit_backoff(attempt: usize) -> Duration {
    Duration::from_secs(2 * (attempt as u64 + 1))
}

fn parse_subreddit_info(root: &Value) -> Result<RedditSubredditInfo, MacosError> {
    let data = object(root, "reddit about payload")?
        .get("data")
        .ok_or_else(|| MacosError::Other("invalid reddit json payload: missing about data".to_string()))
        .and_then(|value| object(value, "reddit about data"))?;
    Ok(RedditSubredditInfo {
        name: string(data, "display_name").unwrap_or_default().to_ascii_lowercase(),
        display_name: string(data, "display_name_prefixed").unwrap_or_default(),
        title: optional_string(data, "title"),
        description: optional_string(data, "public_description"),
        subscribers: optional_u64(data, "subscribers"),
    })
}

fn parse_listing_posts(root: &Value) -> Result<Vec<RedditPostSummary>, MacosError> {
    let children = listing_children(root)?;
    Ok(children
        .iter()
        .filter_map(|child| thing_data(child).ok())
        .filter_map(parse_post_summary)
        .collect())
}

fn parse_post_result(root: &Value) -> Result<RedditPostResult, MacosError> {
    let items = array(root, "reddit post payload")?;
    let post_listing = items
        .first()
        .ok_or_else(|| MacosError::Other("invalid reddit post payload: missing post listing".to_string()))?;
    let comments_listing = items
        .get(1)
        .ok_or_else(|| MacosError::Other("invalid reddit post payload: missing comments listing".to_string()))?;
    let post = listing_children(post_listing)?
        .iter()
        .filter_map(|child| thing_data(child).ok())
        .find_map(parse_post_summary)
        .ok_or_else(|| MacosError::Other("post payload missing article data".to_string()))?;
    let comments = listing_children(comments_listing)?
        .iter()
        .filter(|child| child.get("kind").and_then(Value::as_str) == Some("t1"))
        .filter_map(|child| thing_data(child).ok())
        .filter_map(parse_comment)
        .collect();
    Ok(RedditPostResult { post, comments })
}

fn listing_children(root: &Value) -> Result<&Vec<Value>, MacosError> {
    let data = object(root, "reddit listing payload")?
        .get("data")
        .ok_or_else(|| MacosError::Other("invalid reddit json payload: missing listing data".to_string()))
        .and_then(|value| object(value, "reddit listing data"))?;
    data.get("children")
        .ok_or_else(|| MacosError::Other("invalid reddit json payload: missing listing children".to_string()))
        .and_then(|value| array(value, "reddit listing children"))
}

fn thing_data(root: &Value) -> Result<&Map<String, Value>, MacosError> {
    object(root, "reddit thing")?
        .get("data")
        .ok_or_else(|| MacosError::Other("invalid reddit thing: missing data".to_string()))
        .and_then(|value| object(value, "reddit thing data"))
}

fn parse_post_summary(data: &Map<String, Value>) -> Option<RedditPostSummary> {
    Some(RedditPostSummary {
        id: string(data, "id")?,
        title: string(data, "title")?,
        author: string(data, "author")?,
        subreddit: string(data, "subreddit")?,
        url: string(data, "url")?,
        permalink: prefixed_permalink(optional_string(data, "permalink")?)?,
        score: optional_i64(data, "score").unwrap_or(0),
        num_comments: optional_u64(data, "num_comments").unwrap_or(0),
        created_utc: optional_i64(data, "created_utc").unwrap_or(0),
        selftext: optional_string(data, "selftext").filter(|value| !value.is_empty()),
    })
}

fn parse_comment(data: &Map<String, Value>) -> Option<RedditComment> {
    Some(RedditComment {
        id: string(data, "id")?,
        author: string(data, "author")?,
        body: string(data, "body")?,
        score: optional_i64(data, "score").unwrap_or(0),
        created_utc: optional_i64(data, "created_utc").unwrap_or(0),
        permalink: prefixed_permalink(optional_string(data, "permalink")?)?,
    })
}

fn prefixed_permalink(path: String) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    Some(if path.starts_with("http://") || path.starts_with("https://") {
        path
    } else {
        format!("https://reddit.com{path}")
    })
}

fn object<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>, MacosError> {
    value
        .as_object()
        .ok_or_else(|| MacosError::Other(format!("invalid {label}: expected object")))
}

fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>, MacosError> {
    value
        .as_array()
        .ok_or_else(|| MacosError::Other(format!("invalid {label}: expected array")))
}

fn string(data: &Map<String, Value>, key: &str) -> Option<String> {
    data.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn optional_string(data: &Map<String, Value>, key: &str) -> Option<String> {
    string(data, key)
}

fn optional_u64(data: &Map<String, Value>, key: &str) -> Option<u64> {
    data.get(key).and_then(|value| {
        value.as_u64().or_else(|| {
            value
                .as_i64()
                .and_then(|n| if n >= 0 { Some(n as u64) } else { None })
        })
    })
}

fn optional_i64(data: &Map<String, Value>, key: &str) -> Option<i64> {
    data.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().map(|n| n as i64))
            .or_else(|| value.as_f64().map(|n| n as i64))
    })
}

#[cfg(test)]
mod tests;
