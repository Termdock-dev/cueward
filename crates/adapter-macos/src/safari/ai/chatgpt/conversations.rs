use std::time::{Duration, Instant};

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::super::super::core::execute_js_for_profile;
use super::{SafariConversation, chatgpt_list_conversations_js};

pub fn chatgpt_list_conversations(
    profile_filter: Option<&str>,
) -> Result<Vec<SafariConversation>, MacosError> {
    with_safari_session(|| {
        let js = chatgpt_list_conversations_js();
        let deadline = Instant::now() + Duration::from_secs(10);

        while Instant::now() < deadline {
            let raw =
                execute_js_for_profile(&js, profile_filter, "safari_chatgpt_list_conversations")?;
            let items: Vec<SafariConversation> = serde_json::from_str(&raw)
                .map_err(|e| MacosError::Other(format!("failed to parse conversations: {e}")))?;
            if !items.is_empty() {
                return Ok(items);
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        Ok(Vec::new())
    })
}
