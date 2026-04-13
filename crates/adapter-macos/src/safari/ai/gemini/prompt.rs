use std::thread;
use std::time::{Duration, Instant};

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::super::super::core::execute_js_for_profile;
use super::super::{SafariAiResponseResult, should_skip_gemini_response, wait_and_click_send};
use super::{
    build_gemini_click_send_js, build_gemini_fill_input_js, build_gemini_go_home_js,
    gemini_response_extract_js,
};

pub fn ensure_gemini_home(profile_filter: Option<&str>) -> Result<(), MacosError> {
    with_safari_session(|| {
        let _ = execute_js_for_profile(
            &build_gemini_go_home_js(),
            profile_filter,
            "safari_gemini_go_home",
        )?;
        thread::sleep(Duration::from_millis(2500));
        Ok(())
    })
}

pub fn send_gemini_prompt(
    prompt: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiResponseResult, MacosError> {
    with_safari_session(|| {
        let filled = execute_js_for_profile(
            &build_gemini_fill_input_js(prompt),
            profile_filter,
            "safari_gemini_prompt_fill",
        )?;
        if filled.trim() != "true" {
            return Err(MacosError::Other(format!(
                "failed to fill Gemini input: {filled}"
            )));
        }

        wait_and_click_send(
            &build_gemini_click_send_js(),
            profile_filter,
            "safari_gemini_wait_send",
        )?;

        let mut last_text = String::new();
        let mut stable_count = 0;
        let deadline = Instant::now() + Duration::from_secs(60);
        let response_js = gemini_response_extract_js();

        while Instant::now() < deadline {
            thread::sleep(Duration::from_secs(1));
            let text =
                execute_js_for_profile(&response_js, profile_filter, "safari_gemini_response")?;
            let trimmed = text.trim();
            if should_skip_gemini_response(trimmed, prompt) {
                continue;
            }

            if trimmed == last_text {
                stable_count += 1;
                if stable_count >= 2 {
                    return Ok(SafariAiResponseResult {
                        provider: "gemini".to_string(),
                        status: "complete".to_string(),
                        response: trimmed.to_string(),
                        conversation_url: None,
                    });
                }
            } else {
                last_text = trimmed.to_string();
                stable_count = 0;
            }
        }

        if !last_text.is_empty() {
            return Ok(SafariAiResponseResult {
                provider: "gemini".to_string(),
                status: "timeout".to_string(),
                response: last_text,
                conversation_url: None,
            });
        }

        Err(MacosError::Other(
            "timeout waiting for Gemini response".to_string(),
        ))
    })
}
