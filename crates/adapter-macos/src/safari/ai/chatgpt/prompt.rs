use std::time::{Duration, Instant};

use serde_json::Value;

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::super::super::core::execute_js_for_profile;
use super::super::{
    SafariAiImageResult, SafariAiResponseResult, should_skip_chatgpt_response, wait_and_click_send,
};
use super::{
    build_chatgpt_click_send_js, build_chatgpt_fill_input_js, build_chatgpt_go_home_js,
    chatgpt_response_extract_js, poll_chatgpt_images,
};

pub fn ensure_chatgpt_home(profile_filter: Option<&str>) -> Result<(), MacosError> {
    with_safari_session(|| {
        let _ = execute_js_for_profile(
            &build_chatgpt_go_home_js(),
            profile_filter,
            "safari_chatgpt_go_home",
        )?;
        std::thread::sleep(Duration::from_millis(2500));
        Ok(())
    })
}

pub fn send_chatgpt_prompt(
    prompt: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiResponseResult, MacosError> {
    with_safari_session(|| {
        let filled = execute_js_for_profile(
            &build_chatgpt_fill_input_js(prompt),
            profile_filter,
            "safari_chatgpt_prompt_fill",
        )?;
        if filled.trim() != "true" {
            return Err(MacosError::Other(format!(
                "failed to fill ChatGPT input: {filled}"
            )));
        }

        wait_and_click_send(
            &build_chatgpt_click_send_js(),
            profile_filter,
            "safari_chatgpt_wait_send",
        )?;

        let deadline = Instant::now() + Duration::from_secs(120);
        let response_js = chatgpt_response_extract_js();
        let mut last_response = String::new();
        let mut last_conversation_url: Option<String> = None;

        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(750));
            let payload =
                execute_js_for_profile(&response_js, profile_filter, "safari_chatgpt_response")?;
            let value: Value = serde_json::from_str(&payload).map_err(|e| {
                MacosError::Other(format!("failed to parse chatgpt response payload: {e}"))
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

            let should_skip = should_skip_chatgpt_response(response, prompt);
            if !should_skip {
                last_response = response.to_string();
            }

            if status == "complete" {
                return Ok(SafariAiResponseResult {
                    provider: "chatgpt".to_string(),
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
            provider: "chatgpt".to_string(),
            status: "timeout".to_string(),
            response: last_response,
            conversation_url: last_conversation_url,
        })
    })
}

pub fn send_chatgpt_image_prompt(
    prompt: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiImageResult, MacosError> {
    with_safari_session(|| {
        let filled = execute_js_for_profile(
            &build_chatgpt_fill_input_js(prompt),
            profile_filter,
            "safari_chatgpt_image_fill",
        )?;
        if filled.trim() != "true" {
            return Err(MacosError::Other(format!(
                "failed to fill ChatGPT image prompt: {filled}"
            )));
        }

        wait_and_click_send(
            &build_chatgpt_click_send_js(),
            profile_filter,
            "safari_chatgpt_wait_send",
        )?;
        poll_chatgpt_images(180, 3, profile_filter)
    })
}
