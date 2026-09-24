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
    timeout_seconds: u64,
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

        let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
        let response_js = chatgpt_response_extract_js();
        let mut last_response = String::new();
        let mut last_conversation_url: Option<String> = None;
        let mut completion_candidate: Option<String> = None;
        let mut matching_completion_polls = 0;

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

            if status == "complete" && !should_skip {
                if completion_candidate.as_deref() == Some(response) {
                    matching_completion_polls += 1;
                } else {
                    completion_candidate = Some(response.to_string());
                    matching_completion_polls = 1;
                }
            } else {
                completion_candidate = None;
                matching_completion_polls = 0;
            }

            if matching_completion_polls >= 2 {
                return Ok(SafariAiResponseResult {
                    provider: "chatgpt".to_string(),
                    status: "complete".to_string(),
                    response: response.to_string(),
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
    timeout_seconds: u64,
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
        poll_chatgpt_images(timeout_seconds, 3, profile_filter)
    })
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::chatgpt_response_extract_js;

    #[test]
    fn chinese_stop_response_keeps_prompt_running() {
        let script = format!(
            r#"
            globalThis.window = {{location: {{href: 'https://chatgpt.com/c/test'}}}};
            const stop = {{getAttribute: name => name === 'aria-label' ? '停止回應' : null,
                           innerText: '', textContent: ''}};
            const assistant = {{innerText: 'Pro 思考', textContent: 'Pro 思考'}};
            globalThis.document = {{querySelectorAll: selector =>
              selector === '[data-message-author-role="assistant"]' ? [assistant] :
              selector === 'button,[role="button"]' ? [stop] : []}};
            process.stdout.write({script});
            "#,
            script = chatgpt_response_extract_js()
        );
        let output = Command::new("node")
            .arg("-e")
            .arg(script)
            .output()
            .expect("Node.js is required for Safari JavaScript behavior tests");
        assert!(
            output.status.success(),
            "response extractor failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let payload: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("parse response payload");
        assert_eq!(payload["status"], "running");
    }
}
