use std::thread;
use std::time::{Duration, Instant};

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::super::core::{execute_js_for_profile, focus_tab, open};
use super::super::script::escape_js_string;
use super::SocialFeedPost;

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

pub fn x_extract_feed(profile_filter: Option<&str>) -> Result<Vec<SocialFeedPost>, MacosError> {
    with_safari_session(|| {
        let _ = focus_tab("x.com", profile_filter);
        poll_x_posts(5, profile_filter)
    })
}

pub fn x_search(
    query: &str,
    profile_filter: Option<&str>,
) -> Result<Vec<SocialFeedPost>, MacosError> {
    with_safari_session(|| {
        let url = x_search_url(query);
        navigate_tab_or_open(&url, "x.com", profile_filter, "safari_x_search_navigate")?;
        poll_x_posts(10, profile_filter)
    })
}

pub fn x_read_post(
    url: &str,
    profile_filter: Option<&str>,
) -> Result<Vec<SocialFeedPost>, MacosError> {
    with_safari_session(|| {
        navigate_tab_or_open(url, "x.com", profile_filter, "safari_x_read_navigate")?;
        poll_x_posts(10, profile_filter)
    })
}

#[cfg(test)]
mod tests {
    use super::{x_extract_feed_js, x_search_url};

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
}
