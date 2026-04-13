use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::super::core::{execute_js_for_profile, focus_tab};
use super::SocialFeedPost;

pub fn threads_extract_feed(
    profile_filter: Option<&str>,
) -> Result<Vec<SocialFeedPost>, MacosError> {
    with_safari_session(|| {
        let _ = focus_tab("threads.com", profile_filter);

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
        let raw = execute_js_for_profile(js, profile_filter, "safari_threads_feed")?;
        let posts: Vec<SocialFeedPost> = serde_json::from_str(&raw)
            .map_err(|e| MacosError::Other(format!("failed to parse threads feed: {e}")))?;
        Ok(posts)
    })
}
