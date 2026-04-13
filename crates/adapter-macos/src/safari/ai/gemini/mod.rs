use serde_json::Value;

use crate::MacosError;

use super::super::core::execute_js_for_profile;
use super::super::script::escape_js_string;
use super::{GeminiMode, SafariDeepResearchResult};

mod conversations;
mod deep_research;
mod media;
mod prompt;

pub use conversations::{gemini_list_conversations, gemini_read_conversation};
pub use deep_research::{
    poll_gemini_deep_research, prepare_gemini_mode, start_gemini_deep_research,
};
pub use media::{gemini_save_images, gemini_save_media};
pub use prompt::{ensure_gemini_home, send_gemini_prompt};

pub(super) fn gemini_mode_placeholders(mode: GeminiMode) -> &'static [&'static str] {
    match mode {
        GeminiMode::Image => &[
            "請輸入圖片說明",
            "Describe the image",
            "Describe the image you want to create",
        ],
        GeminiMode::DeepResearch => &["你想研究什麼？", "What do you want to research?"],
        GeminiMode::Video => &["描述影片", "Describe the video", "Describe your video"],
        GeminiMode::Music => &[
            "描述音樂",
            "描述要創作的音樂",
            "Describe the music",
            "Describe the music you",
        ],
    }
}

pub(super) fn gemini_mode_slug(mode: GeminiMode) -> &'static str {
    match mode {
        GeminiMode::Image => "image",
        GeminiMode::DeepResearch => "deep-research",
        GeminiMode::Video => "video",
        GeminiMode::Music => "music",
    }
}

pub(super) fn gemini_mode_url(mode: GeminiMode) -> &'static str {
    match mode {
        GeminiMode::Image => "https://gemini.google.com/image",
        GeminiMode::DeepResearch => "https://gemini.google.com/deepresearch",
        GeminiMode::Video => "https://gemini.google.com/veo",
        GeminiMode::Music => "https://gemini.google.com/music",
    }
}

pub(super) fn build_gemini_go_home_js() -> String {
    r#"(function() {
        window.location.href = "https://gemini.google.com/app";
        return "true";
    })()"#
        .to_string()
}

pub(super) fn build_gemini_placeholder_read_js() -> String {
    r#"(function() {
        var input = document.querySelector(".ql-editor, rich-textarea .ProseMirror, div[role='textbox'][contenteditable='true'], div[contenteditable='true']");
        if (!input) return "";
        return String(
          input.getAttribute("data-placeholder") ||
          input.getAttribute("placeholder") ||
          input.getAttribute("aria-label") ||
          ""
        );
    })()"#
        .to_string()
}

pub(super) fn gemini_response_extract_js() -> String {
    r#"(() => {
        const selectors = [
          ".model-response-text",
          ".message-content",
          ".markdown",
          "div[data-test-id='message-content']"
        ];
        for (const selector of selectors) {
          const elements = document.querySelectorAll(selector);
          if (!elements.length) continue;
          const last = elements[elements.length - 1];
          const text = (last.innerText || last.textContent || "").trim();
          if (text) return text;
        }
        return "";
    })()"#
        .to_string()
}

pub(super) fn build_gemini_fill_input_js(prompt: &str) -> String {
    let prompt = escape_js_string(prompt);
    format!(
        r#"(() => {{
            const input = document.querySelector(
              ".ql-editor, rich-textarea .ProseMirror, div[role='textbox'][contenteditable='true'], div[contenteditable='true']"
            );
            if (!input) throw new Error("gemini input not found");
            input.focus();
            input.textContent = "";
            document.execCommand("insertText", false, "{prompt}");
            return "true";
        }})()"#
    )
}

pub(super) fn build_gemini_click_send_js() -> String {
    r#"(() => {
        const sendLabels = ["傳送訊息", "Send message"];
        const buttons = [...document.querySelectorAll('button,[role="button"]')];
        for (const button of buttons) {
          const label = [
            button.getAttribute("aria-label"),
            button.getAttribute("title"),
            button.innerText,
            button.textContent
          ].filter(Boolean).join(" ");
          if (!sendLabels.some((v) => label.includes(v))) continue;
          if (button.disabled) return "disabled";
          button.click();
          return "true";
        }
        return "false";
    })()"#
        .to_string()
}

pub(super) fn gemini_deep_research_poll_js() -> String {
    r#"(() => {
        const text = document.body.innerText || "";
        const selectors = [
          ".model-response-text",
          ".message-content",
          ".markdown",
          "div[data-test-id='message-content']"
        ];
        const extractLastResponse = () => {
          for (const selector of selectors) {
            const elements = document.querySelectorAll(selector);
            if (!elements.length) continue;
            const last = elements[elements.length - 1];
            const content = (last.innerText || last.textContent || "").trim();
            if (content) return content;
          }
          return "";
        };

        if (text.includes("開始研究") || text.includes("Start research") || text.includes("編輯計畫") || text.includes("Edit plan")) {
          return JSON.stringify({
            status: "plan_ready",
            plan: extractLastResponse(),
            actions: ["confirm", "edit"]
          });
        }
        if (text.includes("研究中") || text.includes("Researching") || text.includes("Generating report")) {
          return JSON.stringify({ status: "running" });
        }

        const content = extractLastResponse();
        if (content) {
          return JSON.stringify({ status: "complete", response: content });
        }

        return JSON.stringify({ status: "running" });
    })()"#
        .to_string()
}

pub(super) fn parse_deep_research_payload(
    payload: &str,
) -> Result<SafariDeepResearchResult, MacosError> {
    let value: Value = serde_json::from_str(payload).map_err(|error| {
        MacosError::Other(format!("invalid deep research payload: {error}: {payload}"))
    })?;
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| MacosError::Other("deep research payload missing status".to_string()))?;
    let actions = value
        .get("actions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(SafariDeepResearchResult {
        provider: "gemini".to_string(),
        mode: "deep-research".to_string(),
        status: status.to_string(),
        conversation_url: None,
        plan: value
            .get("plan")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        response: value
            .get("response")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        actions,
    })
}

pub(super) fn get_gemini_conversation_url(
    profile_filter: Option<&str>,
) -> Result<String, MacosError> {
    let js = r#"(() => {
        const url = window.location.href || "";
        if (url.match(/gemini\.google\.com\/app\/[a-f0-9]{10,}/)) return url;
        return "";
    })()"#;
    let url = execute_js_for_profile(js, profile_filter, "safari_gemini_conversation_url")?;
    let url = url.trim();
    if url.is_empty() {
        return Err(MacosError::Other("no conversation URL found".to_string()));
    }
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::{build_gemini_fill_input_js, build_gemini_go_home_js, gemini_response_extract_js};

    #[test]
    fn gemini_fill_input_script_targets_editor() {
        let script = build_gemini_fill_input_js("hello world");

        assert!(script.contains(".ql-editor"));
        assert!(script.contains("hello world"));
        assert!(script.contains("execCommand"));
    }

    #[test]
    fn gemini_go_home_script_targets_root_app() {
        let script = build_gemini_go_home_js();

        assert!(script.contains("gemini.google.com/app"));
    }

    #[test]
    fn gemini_response_extract_script_targets_latest_response() {
        let script = gemini_response_extract_js();

        assert!(script.contains(".model-response-text"));
        assert!(script.contains("querySelectorAll"));
        assert!(script.contains("elements[elements.length - 1]"));
    }
}
