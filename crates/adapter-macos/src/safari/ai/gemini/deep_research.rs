use std::thread;
use std::time::{Duration, Instant};

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::super::super::core::execute_js_for_profile;
use super::super::{
    GeminiMode, SafariAiReadyResult, SafariDeepResearchResult, wait_and_click_send,
};
use super::{
    build_gemini_click_send_js, build_gemini_fill_input_js, build_gemini_placeholder_read_js,
    gemini_deep_research_poll_js, gemini_mode_placeholders, gemini_mode_slug, gemini_mode_url,
    get_gemini_conversation_url, parse_deep_research_payload,
};

pub fn prepare_gemini_mode(
    mode: GeminiMode,
    profile_filter: Option<&str>,
) -> Result<SafariAiReadyResult, MacosError> {
    with_safari_session(|| {
        let url = gemini_mode_url(mode);
        let nav_js =
            format!(r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,);
        let _ = execute_js_for_profile(&nav_js, profile_filter, "safari_gemini_mode_navigate")?;
        thread::sleep(Duration::from_millis(2500));

        let placeholder = execute_js_for_profile(
            &build_gemini_placeholder_read_js(),
            profile_filter,
            "safari_gemini_mode_placeholder",
        )?;
        if !gemini_mode_placeholders(mode)
            .iter()
            .any(|value| placeholder.contains(value))
        {
            return Err(MacosError::Other(format!(
                "unexpected Gemini placeholder after URL navigation to {url}: {placeholder}"
            )));
        }

        Ok(SafariAiReadyResult {
            provider: "gemini".to_string(),
            mode: gemini_mode_slug(mode).to_string(),
            status: "ready".to_string(),
        })
    })
}

pub fn start_gemini_deep_research(
    prompt: &str,
    auto_confirm: bool,
    profile_filter: Option<&str>,
) -> Result<SafariDeepResearchResult, MacosError> {
    with_safari_session(|| {
        prepare_gemini_mode(GeminiMode::DeepResearch, profile_filter)?;

        let filled = execute_js_for_profile(
            &build_gemini_fill_input_js(prompt),
            profile_filter,
            "safari_gemini_deep_research_fill",
        )?;
        if filled.trim() != "true" {
            return Err(MacosError::Other(format!(
                "failed to fill deep research input: {filled}"
            )));
        }

        wait_and_click_send(
            &build_gemini_click_send_js(),
            profile_filter,
            "safari_gemini_wait_send",
        )?;

        thread::sleep(Duration::from_millis(1000));
        let conv_url = get_gemini_conversation_url(profile_filter).ok();

        let mut result = poll_gemini_deep_research(30, profile_filter)?;
        result.conversation_url = conv_url.clone();
        if result.plan.is_none() {
            result.plan = Some(prompt.to_string());
        }

        if auto_confirm && result.status == "plan_ready" {
            let filled = execute_js_for_profile(
                &build_gemini_fill_input_js("ok"),
                profile_filter,
                "safari_gemini_deep_research_confirm_fill",
            )?;
            if filled.trim() != "true" {
                return Err(MacosError::Other("failed to fill confirm text".to_string()));
            }
            wait_and_click_send(
                &build_gemini_click_send_js(),
                profile_filter,
                "safari_gemini_wait_send",
            )?;
            thread::sleep(Duration::from_secs(3));
            let mut final_result = poll_gemini_deep_research(900, profile_filter)?;
            final_result.conversation_url = conv_url;
            Ok(final_result)
        } else {
            Ok(result)
        }
    })
}

pub fn poll_gemini_deep_research(
    timeout_seconds: u64,
    profile_filter: Option<&str>,
) -> Result<SafariDeepResearchResult, MacosError> {
    with_safari_session(|| {
        let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
        let js = gemini_deep_research_poll_js();

        while Instant::now() < deadline {
            let payload =
                execute_js_for_profile(&js, profile_filter, "safari_gemini_deep_research_poll")?;
            let result = parse_deep_research_payload(&payload)?;
            match result.status.as_str() {
                "plan_ready" | "complete" => return Ok(result),
                "running" => {}
                _ => {
                    return Err(MacosError::Other(format!(
                        "unknown deep research status: {}",
                        result.status
                    )));
                }
            }

            thread::sleep(Duration::from_secs(1));
        }

        Ok(SafariDeepResearchResult {
            provider: "gemini".to_string(),
            mode: "deep-research".to_string(),
            status: "timeout".to_string(),
            conversation_url: None,
            plan: None,
            response: None,
            actions: Vec::new(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{gemini_deep_research_poll_js, parse_deep_research_payload};

    #[test]
    fn gemini_deep_research_poll_script_exposes_status_markers() {
        let script = gemini_deep_research_poll_js();

        assert!(script.contains("plan_ready"));
        assert!(script.contains("running"));
        assert!(script.contains("complete"));
        assert!(script.contains("開始研究"));
        assert!(script.contains("編輯計畫"));
    }

    #[test]
    fn parse_deep_research_payload_reads_plan_and_actions() {
        let payload = r#"{"status":"plan_ready","plan":"Outline","actions":["confirm","edit"]}"#;
        let result = parse_deep_research_payload(payload).expect("parse payload");

        assert_eq!(result.status, "plan_ready");
        assert_eq!(result.plan.as_deref(), Some("Outline"));
        assert_eq!(
            result.actions,
            vec!["confirm".to_string(), "edit".to_string()]
        );
    }

    #[test]
    fn parse_deep_research_payload_reads_running_without_plan() {
        let payload = r#"{"status":"running"}"#;
        let result = parse_deep_research_payload(payload).expect("parse payload");

        assert_eq!(result.status, "running");
        assert_eq!(result.plan, None);
        assert_eq!(result.response, None);
        assert!(result.actions.is_empty());
    }
}
