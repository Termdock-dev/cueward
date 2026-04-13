use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::MacosError;

use super::core::execute_js_for_profile;

mod chatgpt;
mod gemini;
mod grok;

pub use chatgpt::{
    chatgpt_list_conversations, chatgpt_save_images, ensure_chatgpt_home,
    send_chatgpt_image_prompt, send_chatgpt_prompt,
};
pub use gemini::{
    ensure_gemini_home, gemini_list_conversations, gemini_read_conversation, gemini_save_images,
    gemini_save_media, poll_gemini_deep_research, prepare_gemini_mode, send_gemini_prompt,
    start_gemini_deep_research,
};
pub use grok::{
    ensure_grok_home, grok_list_conversations, grok_read_conversation, send_grok_prompt,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeminiMode {
    Image,
    DeepResearch,
    Video,
    Music,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariAiReadyResult {
    pub provider: String,
    pub mode: String,
    pub status: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariAiResponseResult {
    pub provider: String,
    pub status: String,
    pub response: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SafariAiImage {
    pub url: String,
    #[serde(default)]
    pub loaded: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SafariAiImageResult {
    pub provider: String,
    pub mode: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub images: Vec<SafariAiImage>,
}

#[derive(Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SafariConversation {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariDeepResearchResult {
    pub provider: String,
    pub mode: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub actions: Vec<String>,
}

pub(super) fn should_skip_gemini_response(trimmed: &str, prompt: &str) -> bool {
    trimmed.is_empty() || trimmed == prompt.trim()
}

pub(super) fn should_skip_chatgpt_response(trimmed: &str, prompt: &str) -> bool {
    trimmed.is_empty() || trimmed == prompt.trim()
}

pub(super) fn should_skip_grok_response(trimmed: &str, prompt: &str) -> bool {
    trimmed.is_empty() || trimmed == prompt.trim()
}

pub(super) fn wait_and_click_send(
    js: &str,
    profile_filter: Option<&str>,
    context: &str,
) -> Result<(), MacosError> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
        let result = execute_js_for_profile(js, profile_filter, context)?;
        if result.trim() == "true" {
            return Ok(());
        }
    }
    Err(MacosError::Other(
        "send button not found or disabled after 5s".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        should_skip_chatgpt_response, should_skip_gemini_response, should_skip_grok_response,
    };

    #[test]
    fn should_skip_gemini_response_trims_prompt_whitespace() {
        assert!(should_skip_gemini_response("hello", "  hello  "));
        assert!(!should_skip_gemini_response("world", "  hello  "));
    }

    #[test]
    fn should_skip_chatgpt_response_trims_prompt_whitespace() {
        assert!(should_skip_chatgpt_response("hello", "  hello  "));
        assert!(!should_skip_chatgpt_response("world", "  hello  "));
    }

    #[test]
    fn should_skip_grok_response_trims_prompt_whitespace() {
        assert!(should_skip_grok_response("hello", "  hello  "));
        assert!(!should_skip_grok_response("world", "  hello  "));
    }
}
