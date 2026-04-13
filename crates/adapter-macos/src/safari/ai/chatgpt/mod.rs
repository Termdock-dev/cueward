use std::time::{Duration, Instant};

use serde_json::Value;

use crate::MacosError;

use super::super::core::execute_js_for_profile;
use super::super::script::escape_js_string;
use super::{SafariAiImage, SafariAiImageResult, SafariConversation};

mod conversations;
mod images;
mod prompt;

pub use conversations::chatgpt_list_conversations;
pub use images::chatgpt_save_images;
pub use prompt::{ensure_chatgpt_home, send_chatgpt_image_prompt, send_chatgpt_prompt};

pub(super) fn build_chatgpt_go_home_js() -> String {
    r#"(function() {
        window.location.href = "https://chatgpt.com/";
        return "true";
    })()"#
        .to_string()
}

pub(super) fn build_chatgpt_fill_input_js(prompt: &str) -> String {
    let prompt = escape_js_string(prompt);
    format!(
        r##"(() => {{
            const input = document.querySelector(
              "#prompt-textarea, textarea[data-id], div[contenteditable='true'][role='textbox'], div[contenteditable='true']"
            );
            if (!input) throw new Error("chatgpt input not found");
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

pub(super) fn build_chatgpt_click_send_js() -> String {
    r#"(() => {
        const sendLabels = ["Send prompt", "Send message", "傳送訊息", "傳送提示"];
        const buttons = [...document.querySelectorAll('button,[role="button"]')];
        for (const button of buttons) {
          const label = [
            button.getAttribute("aria-label"),
            button.getAttribute("title"),
            button.getAttribute("data-testid"),
            button.innerText,
            button.textContent
          ].filter(Boolean).join(" ");
          if (!sendLabels.some((v) => label.includes(v))) continue;
          if (button.disabled || button.getAttribute("aria-disabled") == "true") return "disabled";
          button.click();
          return "true";
        }
        return "false";
    })()"#
        .to_string()
}

pub(super) fn chatgpt_response_extract_js() -> String {
    r#"(() => {
        const getConversationUrl = () => {
          const url = window.location.href || "";
          return /chatgpt\.com\/c\/[a-z0-9-]+/i.test(url) ? url : null;
        };
        const extractText = (node) => (node?.innerText || node?.textContent || "").trim();
        const selectors = [
          '[data-message-author-role="assistant"]',
          'article[data-testid^="conversation-turn-"]',
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
        const isRunning = /Stop generating|Stop streaming|停止生成|停止串流/.test(controls);

        return JSON.stringify({
          status: response ? (isRunning ? "running" : "complete") : "running",
          response,
          conversation_url: getConversationUrl()
        });
    })()"#
        .to_string()
}

pub(super) fn chatgpt_image_list_js() -> String {
    r#"(() => {
        const getConversationUrl = () => {
          const url = window.location.href || "";
          return /chatgpt\.com\/c\/[a-z0-9-]+/i.test(url) ? url : null;
        };
        const candidates = [...document.querySelectorAll("img")]
          .map((img) => ({
            alt: img.alt || "",
            src: img.src || "",
            width: img.naturalWidth || img.width || 0,
            height: img.naturalHeight || img.height || 0,
            loaded: Boolean(img.complete && img.naturalWidth > 0 && img.naturalHeight > 0)
          }))
          .filter((img) => {
            if (!img.src) return false;
            if (!/^https:\/\/chatgpt\.com\/backend-api\/estuary\/content/.test(img.src)) return false;
            if (img.width < 256 || img.height < 256) return false;
            return true;
          });

        const deduped = [];
        const seen = new Set();
        for (const img of candidates) {
          if (seen.has(img.src)) continue;
          seen.add(img.src);
          deduped.push({ url: img.src, loaded: img.loaded });
        }

        const controls = [...document.querySelectorAll('button,[role="button"]')]
          .map((button) => [
            button.getAttribute("aria-label"),
            button.getAttribute("title"),
            button.innerText,
            button.textContent
          ].filter(Boolean).join(" "))
          .join(" ");
        const isRunning = /Stop generating|Stop streaming|停止生成|停止串流/.test(controls);

        return JSON.stringify({
          status: isRunning ? "running" : "complete",
          conversation_url: getConversationUrl(),
          images: deduped
        });
    })()"#
        .to_string()
}

pub(super) fn poll_chatgpt_images(
    timeout_seconds: u64,
    empty_complete_grace_seconds: u64,
    profile_filter: Option<&str>,
) -> Result<SafariAiImageResult, MacosError> {
    let started_at = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    let image_js = chatgpt_image_list_js();
    let mut last_result = SafariAiImageResult {
        provider: "chatgpt".to_string(),
        mode: "image".to_string(),
        status: "running".to_string(),
        conversation_url: None,
        images: Vec::new(),
    };

    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_secs(1));
        let payload =
            execute_js_for_profile(&image_js, profile_filter, "safari_chatgpt_image_poll")?;
        let result = parse_chatgpt_image_payload(&payload)?;
        if result.conversation_url.is_some() {
            last_result.conversation_url = result.conversation_url.clone();
        }
        if !result.images.is_empty() {
            last_result.images = result.images.clone();
        }
        last_result.status = result.status.clone();

        if chatgpt_image_result_is_ready(&result) {
            return Ok(result);
        }

        if chatgpt_should_return_empty_complete_result(
            &result,
            started_at.elapsed(),
            empty_complete_grace_seconds,
        ) {
            return Ok(last_result);
        }
    }

    last_result.status = "timeout".to_string();
    Ok(last_result)
}

pub(super) fn chatgpt_image_extract_js(url: &str) -> String {
    let url = escape_js_string(url);
    format!(
        r#"(() => {{
            const target = "{url}";
            if (!target) return "";
            const img = [...document.querySelectorAll("img")].find((node) => (node.src || "") === target);
            if (!img) return "";
            const canvas = document.createElement("canvas");
            canvas.width = img.naturalWidth || img.width;
            canvas.height = img.naturalHeight || img.height;
            const ctx = canvas.getContext("2d");
            if (!ctx) return "";
            ctx.drawImage(img, 0, 0);
            const dataUrl = canvas.toDataURL("image/png");
            const idx = dataUrl.indexOf(",");
            return idx >= 0 ? dataUrl.substring(idx + 1) : "";
        }})()"#
    )
}

pub(super) fn chatgpt_image_result_is_ready(result: &SafariAiImageResult) -> bool {
    result.status == "complete"
        && !result.images.is_empty()
        && result.images.iter().all(|image| image.loaded)
}

pub(super) fn chatgpt_should_return_empty_complete_result(
    result: &SafariAiImageResult,
    elapsed: Duration,
    empty_complete_grace_seconds: u64,
) -> bool {
    result.status == "complete"
        && result.images.is_empty()
        && elapsed >= Duration::from_secs(empty_complete_grace_seconds)
}

pub(super) fn parse_chatgpt_image_payload(
    payload: &str,
) -> Result<SafariAiImageResult, MacosError> {
    let value: Value = serde_json::from_str(payload).map_err(|error| {
        MacosError::Other(format!("invalid chatgpt image payload: {error}: {payload}"))
    })?;
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("running")
        .to_string();
    let conversation_url = value
        .get("conversation_url")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let images = value
        .get("images")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let url = item.get("url")?.as_str()?;
                    let loaded = item.get("loaded").and_then(Value::as_bool).unwrap_or(false);
                    Some(SafariAiImage {
                        url: url.to_string(),
                        loaded,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(SafariAiImageResult {
        provider: "chatgpt".to_string(),
        mode: "image".to_string(),
        status,
        conversation_url,
        images,
    })
}

pub(super) fn chatgpt_list_conversations_js() -> String {
    r#"(() => {
        const navLabels = [
          "聊天歷程紀錄",
          "Chat history",
          "Conversation history"
        ];
        const nav = navLabels
          .map((label) => document.querySelector(`nav[aria-label="${label}"]`))
          .find(Boolean) || document.querySelector('nav[role="navigation"]');
        if (!nav) return JSON.stringify([]);
        const recentLabels = ["最近", "Recent"];
        const recentButton = Array.from(nav.querySelectorAll("button"))
          .find((button) => recentLabels.includes((button.innerText || button.textContent || "").trim()));
        if (recentButton && recentButton.getAttribute("aria-expanded") === "false") {
          recentButton.click();
        }
        const items = Array.from(nav.querySelectorAll('a[href^="/c/"]'));
        const convos = [];
        const seen = new Set();
        for (const a of items) {
          const href = a.getAttribute("href") || "";
          const title = (a.innerText || a.textContent || a.getAttribute("aria-label") || "").trim();
          if (!href || !title || seen.has(href)) continue;
          seen.add(href);
          convos.push({ title, url: "https://chatgpt.com" + href });
        }
        return JSON.stringify(convos);
    })()"#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        SafariAiImage, SafariAiImageResult, build_chatgpt_click_send_js,
        build_chatgpt_fill_input_js, build_chatgpt_go_home_js, chatgpt_image_extract_js,
        chatgpt_image_list_js, chatgpt_image_result_is_ready, chatgpt_list_conversations_js,
        chatgpt_response_extract_js, chatgpt_should_return_empty_complete_result,
        parse_chatgpt_image_payload,
    };
    use std::time::Duration;

    #[test]
    fn chatgpt_go_home_script_targets_new_chat() {
        let script = build_chatgpt_go_home_js();

        assert!(script.contains("chatgpt.com"));
        assert!(script.contains("window.location.href"));
    }

    #[test]
    fn chatgpt_fill_input_script_targets_prompt_textarea() {
        let script = build_chatgpt_fill_input_js("hello from chatgpt");

        assert!(script.contains("#prompt-textarea"));
        assert!(script.contains("hello from chatgpt"));
        assert!(script.contains("execCommand"));
    }

    #[test]
    fn chatgpt_click_send_script_checks_accessible_labels() {
        let script = build_chatgpt_click_send_js();

        assert!(script.contains("Send prompt"));
        assert!(script.contains("Send message"));
        assert!(script.contains("button.disabled"));
    }

    #[test]
    fn chatgpt_response_extract_script_returns_response_and_conversation_url() {
        let script = chatgpt_response_extract_js();

        assert!(script.contains("article"));
        assert!(script.contains("conversation_url"));
        assert!(script.contains("window.location.href"));
        assert!(script.contains("complete"));
    }

    #[test]
    fn chatgpt_list_script_targets_history_nav_links() {
        let script = chatgpt_list_conversations_js();

        assert!(script.contains("Chat history"));
        assert!(script.contains("nav[role=\"navigation\"]"));
        assert!(script.contains("a[href^=\"/c/\"]"));
        assert!(script.contains("https://chatgpt.com"));
        assert!(script.contains("Recent"));
    }

    #[test]
    fn chatgpt_image_list_script_reads_generated_images() {
        let script = chatgpt_image_list_js();

        assert!(script.contains("chatgpt\\.com\\/backend-api\\/estuary\\/content"));
        assert!(script.contains("conversation_url"));
        assert!(script.contains("img.width < 256"));
    }

    #[test]
    fn chatgpt_image_list_script_includes_loaded_signal() {
        let script = chatgpt_image_list_js();

        assert!(script.contains("img.complete"));
        assert!(script.contains("loaded"));
    }

    #[test]
    fn chatgpt_image_extract_script_targets_url() {
        let script = chatgpt_image_extract_js("https://example.com/img.png");

        assert!(script.contains("https://example.com/img.png"));
        assert!(script.contains("drawImage"));
        assert!(!script.contains("new Set"));
    }

    #[test]
    fn parse_chatgpt_image_payload_reads_images() {
        let payload = r#"{"status":"complete","conversation_url":"https://chatgpt.com/c/test","images":[{"url":"https://chatgpt.com/backend-api/estuary/content?id=file_123"}]}"#;
        let result = parse_chatgpt_image_payload(payload).expect("parse payload");

        assert_eq!(result.status, "complete");
        assert_eq!(
            result.conversation_url.as_deref(),
            Some("https://chatgpt.com/c/test")
        );
        assert_eq!(result.images.len(), 1);
        assert_eq!(
            result.images[0].url,
            "https://chatgpt.com/backend-api/estuary/content?id=file_123"
        );
    }

    #[test]
    fn parse_chatgpt_image_payload_reads_loaded_state() {
        let payload = r#"{"status":"complete","conversation_url":"https://chatgpt.com/c/test","images":[{"url":"https://chatgpt.com/backend-api/estuary/content?id=file_123","loaded":true},{"url":"https://chatgpt.com/backend-api/estuary/content?id=file_456","loaded":false}]}"#;
        let result = parse_chatgpt_image_payload(payload).expect("parse payload");

        assert_eq!(result.images.len(), 2);
        assert!(result.images[0].loaded);
        assert!(!result.images[1].loaded);
    }

    #[test]
    fn chatgpt_image_result_requires_all_images_to_be_ready() {
        let unloaded = SafariAiImageResult {
            provider: "chatgpt".to_string(),
            mode: "image".to_string(),
            status: "complete".to_string(),
            conversation_url: Some("https://chatgpt.com/c/test".to_string()),
            images: vec![SafariAiImage {
                url: "https://chatgpt.com/backend-api/estuary/content?id=file_123".to_string(),
                loaded: false,
            }],
        };
        let mixed = SafariAiImageResult {
            images: vec![
                SafariAiImage {
                    url: "https://chatgpt.com/backend-api/estuary/content?id=file_123".to_string(),
                    loaded: true,
                },
                SafariAiImage {
                    url: "https://chatgpt.com/backend-api/estuary/content?id=file_456".to_string(),
                    loaded: false,
                },
            ],
            ..unloaded.clone()
        };
        let loaded = SafariAiImageResult {
            images: vec![
                SafariAiImage {
                    url: "https://chatgpt.com/backend-api/estuary/content?id=file_123".to_string(),
                    loaded: true,
                },
                SafariAiImage {
                    url: "https://chatgpt.com/backend-api/estuary/content?id=file_456".to_string(),
                    loaded: true,
                },
            ],
            ..unloaded.clone()
        };

        assert!(!chatgpt_image_result_is_ready(&unloaded));
        assert!(!chatgpt_image_result_is_ready(&mixed));
        assert!(chatgpt_image_result_is_ready(&loaded));
    }

    #[test]
    fn chatgpt_empty_complete_grace_only_applies_without_images() {
        let complete_without_images = SafariAiImageResult {
            provider: "chatgpt".to_string(),
            mode: "image".to_string(),
            status: "complete".to_string(),
            conversation_url: Some("https://chatgpt.com/c/test".to_string()),
            images: Vec::new(),
        };
        let complete_with_unloaded_images = SafariAiImageResult {
            images: vec![SafariAiImage {
                url: "https://chatgpt.com/backend-api/estuary/content?id=file_123".to_string(),
                loaded: false,
            }],
            ..complete_without_images.clone()
        };

        assert!(chatgpt_should_return_empty_complete_result(
            &complete_without_images,
            Duration::from_secs(8),
            3
        ));
        assert!(!chatgpt_should_return_empty_complete_result(
            &complete_with_unloaded_images,
            Duration::from_secs(8),
            3
        ));
    }
}
