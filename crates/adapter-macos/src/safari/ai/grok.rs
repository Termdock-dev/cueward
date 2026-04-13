use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::super::core::execute_js_for_profile;
use super::super::script::escape_js_string;
use super::{
    SafariAiResponseResult, SafariConversation, should_skip_grok_response, wait_and_click_send,
};

fn build_grok_go_home_js() -> String {
    r#"(function() {
        window.location.href = "https://grok.com/";
        return "true";
    })()"#
        .to_string()
}

fn build_grok_fill_input_js(prompt: &str) -> String {
    let prompt = escape_js_string(prompt);
    format!(
        r##"(() => {{
            const input = document.querySelector(
              "textarea, textarea[placeholder], div[contenteditable='true'][role='textbox'], div[contenteditable='true']"
            );
            if (!input) throw new Error("grok input not found");
            input.focus();
            if ("value" in input) {{
              input.value = "";
            }} else {{
              input.textContent = "";
            }}
            document.execCommand("insertText", false, "{prompt}");
            return "true";
        }})()"##
    )
}

fn build_grok_click_send_js() -> String {
    r#"(() => {
        const sendLabels = ["send", "ask grok", "提交", "傳送", "送出"];
        const buttons = [...document.querySelectorAll('button,[role="button"]')];
        for (const button of buttons) {
          const label = [
            button.getAttribute("aria-label"),
            button.getAttribute("title"),
            button.getAttribute("data-testid"),
            button.innerText,
            button.textContent
          ].filter(Boolean).join(" ").toLowerCase();
          if (!sendLabels.some((v) => label.includes(v))) continue;
          if (button.disabled || button.getAttribute("aria-disabled") == "true") return "disabled";
          button.click();
          return "true";
        }
        return "false";
    })()"#
        .to_string()
}

fn grok_response_extract_js() -> String {
    r#"(() => {
        const getConversationUrl = () => {
          const url = window.location.href || "";
          if (/grok\.com\/share\//i.test(url)) return url;
          if (/grok\.com\/c\//i.test(url)) return url;
          if (/grok\.com\/history/i.test(url)) return url;
          return null;
        };
        const extractText = (node) => (node?.innerText || node?.textContent || "").trim();
        const selectors = [
          'article',
          '[data-testid*="message"]',
          '[data-testid*="conversation"]',
          'main .prose',
          'main article'
        ];

        let response = "";
        for (const selector of selectors) {
          const nodes = [...document.querySelectorAll(selector)];
          for (let i = nodes.length - 1; i >= 0; i--) {
            const text = extractText(nodes[i]);
            if (text) {
              response = text;
              break;
            }
          }
          if (response) break;
        }

        const controls = [...document.querySelectorAll('button,[role="button"]')]
          .map((button) => [
            button.getAttribute("aria-label"),
            button.getAttribute("title"),
            button.innerText,
            button.textContent
          ].filter(Boolean).join(" "))
          .join(" ");
        const isRunning = /Stop generating|Stop responding|停止生成|停止回應|Thinking\.\.\.|Generating/i.test(controls);

        return JSON.stringify({
          status: response ? (isRunning ? "running" : "complete") : "running",
          response,
          conversation_url: getConversationUrl()
        });
    })()"#
        .to_string()
}

fn grok_list_conversations_js() -> String {
    r#"(() => {
        const items = Array.from(document.querySelectorAll('a[href^="/c/"], a.peer\\/menu-button[href^="/c/"]'));
        const convos = [];
        const seen = new Set();
        for (const a of items) {
          const href = a.getAttribute("href") || "";
          const title = (a.innerText || a.textContent || a.getAttribute("aria-label") || "").trim();
          if (!href || !title) continue;
          const url = href.startsWith("http") ? href : "https://grok.com" + href;
          if (!/grok\.com\/c\//i.test(url)) continue;
          if (seen.has(url)) continue;
          seen.add(url);
          convos.push({ title, url });
        }
        return JSON.stringify(convos);
    })()"#
        .to_string()
}

pub fn ensure_grok_home(profile_filter: Option<&str>) -> Result<(), MacosError> {
    with_safari_session(|| {
        let _ = execute_js_for_profile(
            &build_grok_go_home_js(),
            profile_filter,
            "safari_grok_go_home",
        )?;
        thread::sleep(Duration::from_millis(2500));
        Ok(())
    })
}

pub fn grok_list_conversations(
    profile_filter: Option<&str>,
) -> Result<Vec<SafariConversation>, MacosError> {
    with_safari_session(|| {
        let js = grok_list_conversations_js();
        let deadline = Instant::now() + Duration::from_secs(10);

        while Instant::now() < deadline {
            let raw =
                execute_js_for_profile(&js, profile_filter, "safari_grok_list_conversations")?;
            let items: Vec<SafariConversation> = serde_json::from_str(&raw)
                .map_err(|e| MacosError::Other(format!("failed to parse conversations: {e}")))?;
            if !items.is_empty() {
                return Ok(items);
            }
            thread::sleep(Duration::from_millis(500));
        }

        Ok(Vec::new())
    })
}

pub fn grok_read_conversation(
    url: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiResponseResult, MacosError> {
    with_safari_session(|| {
        let nav_js = format!(
            r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
            url = escape_js_string(url),
        );
        let _ = execute_js_for_profile(&nav_js, profile_filter, "safari_grok_read_navigate")?;
        thread::sleep(Duration::from_millis(3000));

        let deadline = Instant::now() + Duration::from_secs(30);
        let response_js = grok_response_extract_js();
        let mut last_response = String::new();
        let mut last_conversation_url: Option<String> = None;

        while Instant::now() < deadline {
            thread::sleep(Duration::from_millis(750));
            let payload =
                execute_js_for_profile(&response_js, profile_filter, "safari_grok_read_response")?;
            let value: Value = serde_json::from_str(&payload).map_err(|e| {
                MacosError::Other(format!("failed to parse grok response payload: {e}"))
            })?;

            let status = value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("running");
            let response = value
                .get("response")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let conversation_url = value
                .get("conversation_url")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);

            if conversation_url.is_some() {
                last_conversation_url = conversation_url.clone();
            }
            if !response.is_empty() {
                last_response = response.to_string();
            }

            if status == "complete" && !response.is_empty() {
                return Ok(SafariAiResponseResult {
                    provider: "grok".to_string(),
                    status: "complete".to_string(),
                    response: response.to_string(),
                    conversation_url: conversation_url.or_else(|| last_conversation_url.clone()),
                });
            }
        }

        Ok(SafariAiResponseResult {
            provider: "grok".to_string(),
            status: "timeout".to_string(),
            response: last_response,
            conversation_url: last_conversation_url,
        })
    })
}

pub fn send_grok_prompt(
    prompt: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiResponseResult, MacosError> {
    with_safari_session(|| {
        let filled = execute_js_for_profile(
            &build_grok_fill_input_js(prompt),
            profile_filter,
            "safari_grok_prompt_fill",
        )?;
        if filled.trim() != "true" {
            return Err(MacosError::Other(format!(
                "failed to fill Grok input: {filled}"
            )));
        }

        wait_and_click_send(
            &build_grok_click_send_js(),
            profile_filter,
            "safari_grok_wait_send",
        )?;

        let deadline = Instant::now() + Duration::from_secs(120);
        let response_js = grok_response_extract_js();
        let mut last_response = String::new();
        let mut last_conversation_url: Option<String> = None;

        while Instant::now() < deadline {
            thread::sleep(Duration::from_millis(750));
            let payload =
                execute_js_for_profile(&response_js, profile_filter, "safari_grok_response")?;
            let value: Value = serde_json::from_str(&payload).map_err(|e| {
                MacosError::Other(format!("failed to parse grok response payload: {e}"))
            })?;

            let status = value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("running");
            let response = value
                .get("response")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let conversation_url = value
                .get("conversation_url")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            if conversation_url.is_some() {
                last_conversation_url = conversation_url.clone();
            }

            let should_skip = should_skip_grok_response(response, prompt);
            if !should_skip {
                last_response = response.to_string();
            }

            if status == "complete" {
                return Ok(SafariAiResponseResult {
                    provider: "grok".to_string(),
                    status: "complete".to_string(),
                    response: if should_skip {
                        last_response.clone()
                    } else {
                        response.to_string()
                    },
                    conversation_url: conversation_url.or_else(|| last_conversation_url.clone()),
                });
            }
        }

        Ok(SafariAiResponseResult {
            provider: "grok".to_string(),
            status: "timeout".to_string(),
            response: last_response,
            conversation_url: last_conversation_url,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_grok_click_send_js, build_grok_fill_input_js, build_grok_go_home_js,
        grok_list_conversations_js, grok_response_extract_js,
    };

    #[test]
    fn grok_go_home_script_targets_root_page() {
        let script = build_grok_go_home_js();

        assert!(script.contains("https://grok.com/"));
        assert!(script.contains("window.location.href"));
    }

    #[test]
    fn grok_fill_input_script_targets_prompt_editor() {
        let script = build_grok_fill_input_js("hello from grok");

        assert!(script.contains("textarea"));
        assert!(script.contains("contenteditable='true'"));
        assert!(script.contains("hello from grok"));
    }

    #[test]
    fn grok_click_send_script_checks_accessible_labels() {
        let script = build_grok_click_send_js();

        assert!(script.contains("ask grok"));
        assert!(script.contains("提交"));
        assert!(script.contains("aria-label"));
        assert!(script.contains("data-testid"));
    }

    #[test]
    fn grok_response_extract_script_returns_response_and_conversation_url() {
        let script = grok_response_extract_js();

        assert!(script.contains("grok\\.com\\/share\\/"));
        assert!(script.contains("conversation_url"));
        assert!(script.contains("window.location.href"));
        assert!(script.contains("complete"));
    }

    #[test]
    fn grok_list_script_targets_share_and_history_links() {
        let script = grok_list_conversations_js();

        assert!(script.contains("a[href^=\"/c/\"]"));
        assert!(script.contains("a.peer\\\\/menu-button[href^=\"/c/\"]"));
        assert!(script.contains("https://grok.com"));
    }
}
