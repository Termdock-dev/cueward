use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::super::super::core::execute_js_for_profile;
use super::super::super::script::escape_js_string;
use super::super::{SafariAiResponseResult, SafariConversation};

pub fn gemini_list_conversations(
    profile_filter: Option<&str>,
) -> Result<Vec<SafariConversation>, MacosError> {
    with_safari_session(|| {
        let js = r#"(() => {
        const items = document.querySelectorAll('a[href*="/app/"]');
        const convos = [];
        for (const a of items) {
          const href = a.getAttribute("href") || "";
          if (!/\/app\/[a-f0-9]{10,}/.test(href)) continue;
          const title = (a.innerText || a.textContent || "").trim();
          if (!title) continue;
          convos.push({ title, url: "https://gemini.google.com" + href });
        }
        return JSON.stringify(convos);
    })()"#;
        let raw = execute_js_for_profile(js, profile_filter, "safari_gemini_list_conversations")?;
        let items: Vec<SafariConversation> = serde_json::from_str(&raw)
            .map_err(|e| MacosError::Other(format!("failed to parse conversations: {e}")))?;
        Ok(items)
    })
}

pub fn gemini_read_conversation(
    url: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiResponseResult, MacosError> {
    with_safari_session(|| {
        let nav_js = format!(
            r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
            url = escape_js_string(url),
        );
        let _ = execute_js_for_profile(&nav_js, profile_filter, "safari_gemini_read_navigate")?;
        thread::sleep(Duration::from_millis(3000));

        let read_js = r#"(() => {
        const panels = document.querySelectorAll('.markdown.markdown-main-panel');
        let biggest = null;
        let maxLen = 0;
        for (const p of panels) {
          const len = (p.innerText || "").length;
          if (len > maxLen) { maxLen = len; biggest = p; }
        }
        if (!biggest || maxLen === 0) return JSON.stringify({ status: "empty", response: "" });
        return JSON.stringify({ status: "complete", response: (biggest.innerText || "").trim() });
    })()"#;
        let raw = execute_js_for_profile(read_js, profile_filter, "safari_gemini_read_content")?;
        let value: Value = serde_json::from_str(&raw)
            .map_err(|e| MacosError::Other(format!("failed to parse read result: {e}")))?;
        let status = value
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("error");
        let response = value.get("response").and_then(|v| v.as_str()).unwrap_or("");
        Ok(SafariAiResponseResult {
            provider: "gemini".to_string(),
            status: status.to_string(),
            response: response.to_string(),
            conversation_url: None,
        })
    })
}
