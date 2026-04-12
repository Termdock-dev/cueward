use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

use cueward_core::{Cue, CueSource};

use crate::MacosError;
use crate::applescript::{escape, escape_body, run_capture};

/// Core Data epoch: 2001-01-01 00:00:00 UTC
const CORE_DATA_EPOCH: i64 = 978_307_200;
const TAB_SEPARATOR: &str = "---TAB_SEP---";
const FIELD_SEPARATOR: &str = "<<<FIELD_SEP>>>";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariTab {
    pub window_id: i64,
    pub window_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub index: usize,
    pub title: String,
    pub url: String,
    pub active: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariEvalResult {
    pub result: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariReadResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SafariScrollReadChunk {
    pub iteration: u64,
    pub content: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariScrollReadResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    pub times: u64,
    pub chunks: Vec<SafariScrollReadChunk>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariSourceResult {
    pub html: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariCloseResult {
    pub closed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariClickResult {
    pub clicked: bool,
    pub selector: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariFillResult {
    pub filled: bool,
    pub selector: String,
    pub text: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariWaitResult {
    pub found: bool,
    pub selector: String,
    pub timeout_seconds: u64,
}

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

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SocialFeedPost {
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    pub content: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub metrics: Vec<String>,
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

fn history_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Safari/History.db")
}

fn to_core_data_timestamp(dt: DateTime<Utc>) -> f64 {
    (dt.timestamp() - CORE_DATA_EPOCH) as f64
}

fn from_core_data_timestamp(ts: f64) -> DateTime<Utc> {
    Utc.timestamp_opt(ts as i64 + CORE_DATA_EPOCH, 0)
        .single()
        .unwrap_or_default()
}

fn decode_field(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }

        match chars.next() {
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some('t') => decoded.push('\t'),
            Some('s') => decoded.push_str(TAB_SEPARATOR),
            Some('\\') => decoded.push('\\'),
            Some(other) => {
                decoded.push('\\');
                decoded.push(other);
            }
            None => decoded.push('\\'),
        }
    }
    decoded
}

fn escape_js_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn extract_profile(window_name: &str, active_tab_title: &str) -> Option<String> {
    let expected_suffix = format!(" — {active_tab_title}");
    window_name
        .strip_suffix(&expected_suffix)
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .map(ToOwned::to_owned)
}

fn target_window_block(profile_filter: Option<&str>) -> String {
    match profile_filter {
        Some(profile) => {
            let profile = escape(profile);
            format!(
                r#"set w to missing value
            repeat with candidate in every window
                if (name of candidate contains "{profile}") then
                    set w to candidate
                    exit repeat
                end if
            end repeat
            if w is missing value then
                return ""
            end if"#,
            )
        }
        None => "set w to front window".to_string(),
    }
}

fn parse_tab_line(line: &str) -> Option<SafariTab> {
    let parts: Vec<&str> = line.split(FIELD_SEPARATOR).collect();
    if parts.len() != 6 {
        return None;
    }

    let window_id = parts[0].trim().parse().ok()?;
    let window_name = decode_field(parts[1]);
    let index = parts[2].trim().parse().ok()?;
    let title = decode_field(parts[3]);
    let url = decode_field(parts[4]);
    let active = parts[5].trim() == "true";

    Some(SafariTab {
        window_id,
        window_name,
        profile: None,
        index,
        title,
        url,
        active,
    })
}

fn parse_tabs_output(stdout: &str) -> Vec<SafariTab> {
    let mut tabs: Vec<SafariTab> = stdout
        .split(TAB_SEPARATOR)
        .filter(|line| !line.trim().is_empty())
        .filter_map(parse_tab_line)
        .collect();

    let mut profiles_by_window = HashMap::new();
    for tab in &tabs {
        if tab.active {
            if let Some(profile) = extract_profile(&tab.window_name, &tab.title) {
                profiles_by_window.insert(tab.window_id, profile);
            }
        }
    }

    for tab in &mut tabs {
        tab.profile = profiles_by_window.get(&tab.window_id).cloned();
    }

    tabs
}

fn safari_script_prelude() -> String {
    format!(
        r#"
        on replace_text(find_text, replace_text, source_text)
            set previous_delimiters to AppleScript's text item delimiters
            set AppleScript's text item delimiters to find_text
            set chunks to every text item of source_text
            set AppleScript's text item delimiters to replace_text
            set replaced_text to chunks as text
            set AppleScript's text item delimiters to previous_delimiters
            return replaced_text
        end replace_text

        on encode_field(source_text)
            if source_text is missing value then
                return ""
            end if
            set escaped_text to my replace_text("\\", "\\\\", source_text)
            set escaped_text to my replace_text(tab, "\\t", escaped_text)
            set escaped_text to my replace_text(return, "\\r", escaped_text)
            set escaped_text to my replace_text(linefeed, "\\n", escaped_text)
            set escaped_text to my replace_text("{separator}", "\\s", escaped_text)
            return escaped_text
        end encode_field
    "#,
        separator = TAB_SEPARATOR,
    )
}

fn build_tab_return_block(tab_ref: &str, active_flag: &str) -> String {
    format!(
        r#"set winId to id of w
            set winName to my encode_field(name of w)
            set tabIndex to (index of {tab_ref}) - 1
            set tabTitle to my encode_field(name of {tab_ref})
            set tabURL to my encode_field(URL of {tab_ref})
            return (winId as text) & "{field_separator}" & winName & "{field_separator}" & (tabIndex as text) & "{field_separator}" & tabTitle & "{field_separator}" & tabURL & "{field_separator}" & "{active_flag}""#,
        field_separator = FIELD_SEPARATOR,
    )
}

fn build_tabs_script() -> String {
    format!(
        r#"
        {prelude}
        tell application "Safari"
            set output to ""
            repeat with w in every window
                set winId to id of w
                set winName to my encode_field(name of w)
                set activeTabIndex to index of current tab of w
                repeat with t in tabs of w
                    set tabIndex to (index of t) - 1
                    set tabTitle to my encode_field(name of t)
                    set tabURL to my encode_field(URL of t)
                    if (index of t) is activeTabIndex then
                        set isActive to "true"
                    else
                        set isActive to "false"
                    end if
                    set output to output & (winId as text) & "{field_separator}" & winName & "{field_separator}" & (tabIndex as text) & "{field_separator}" & tabTitle & "{field_separator}" & tabURL & "{field_separator}" & isActive & "{separator}"
                end repeat
            end repeat
            return output
        end tell
    "#,
        prelude = safari_script_prelude(),
        separator = TAB_SEPARATOR,
        field_separator = FIELD_SEPARATOR,
    )
}

fn build_active_tab_script(profile_filter: Option<&str>) -> String {
    let tab_return = build_tab_return_block("t", "true");
    let target_window = target_window_block(profile_filter);
    format!(
        r#"
        {prelude}
        tell application "Safari"
            if (count of windows) is 0 then
                return ""
            end if
            {target_window}
            set t to current tab of w
            {tab_return}
        end tell
    "#,
        prelude = safari_script_prelude(),
        target_window = target_window,
        tab_return = tab_return,
    )
}

fn build_exec_script_for_profile(js_code: &str, profile_filter: Option<&str>) -> String {
    let js_expr = escape_body(js_code);
    let target_window = target_window_block(profile_filter);
    format!(
        r#"
        {prelude}
        tell application "Safari"
            if (count of windows) is 0 then
                return ""
            end if
            {target_window}
            set jsCode to {js_expr}
            set rawResult to missing value
            try
                set rawResult to do JavaScript jsCode in current tab of w
            on error errMsg number errNum
                error errMsg number errNum
            end try
            if rawResult is missing value then
                return ""
            end if
            set rawResult to rawResult as string
            return my encode_field(rawResult)
        end tell
    "#,
        prelude = safari_script_prelude(),
        target_window = target_window,
        js_expr = js_expr,
    )
}

fn build_open_script(url: &str, profile_filter: Option<&str>) -> String {
    let escaped_url = escape(url);
    let tab_return = build_tab_return_block("t", "true");
    let target_window = target_window_block(profile_filter);
    format!(
        r#"
        {prelude}
        tell application "Safari"
            if (count of windows) is 0 then
                make new document with properties {{URL:"{escaped_url}"}}
                set w to front window
            else
                {target_window}
                set t to make new tab at end of tabs of w with properties {{URL:"{escaped_url}"}}
                set current tab of w to t
            end if
            delay 0.1
            set t to current tab of w
            {tab_return}
        end tell
    "#,
        prelude = safari_script_prelude(),
        escaped_url = escaped_url,
        target_window = target_window,
        tab_return = tab_return,
    )
}

fn build_close_script(index: Option<usize>) -> String {
    let target_block = match index {
        Some(index) => {
            let one_based = index + 1;
            format!(
                r#"if {one_based} > (count of tabs of w) then
                    error "tab index out of range"
                end if
                set t to tab {one_based} of w"#
            )
        }
        None => "set t to current tab of w".to_string(),
    };

    format!(
        r#"
        tell application "Safari"
            if (count of windows) is 0 then
                return "false"
            end if
            set w to front window
            {target_block}
            close t
            return "true"
        end tell
    "#,
        target_block = target_block,
    )
}

#[allow(dead_code)]
fn build_exec_script(js_code: &str) -> String {
    let js_expr = escape_body(js_code);
    format!(
        r#"
        {prelude}
        tell application "Safari"
            if (count of windows) is 0 then
                return ""
            end if
            set jsCode to {js_expr}
            set rawResult to missing value
            try
                set rawResult to do JavaScript jsCode in current tab of front window
            on error errMsg number errNum
                error errMsg number errNum
            end try
            if rawResult is missing value then
                return ""
            end if
            set rawResult to rawResult as string
            return my encode_field(rawResult)
        end tell
    "#,
        prelude = safari_script_prelude(),
        js_expr = js_expr,
    )
}

fn selector_text_js(selector: &str) -> String {
    let selector = escape_js_string(selector);
    format!(
        r#"(() => {{
            const el = document.querySelector("{selector}");
            if (!el) throw new Error("selector not found");
            return (el.innerText ?? el.textContent ?? "").trim();
        }})()"#
    )
}

fn selector_exists_js(selector: &str) -> String {
    let selector = escape_js_string(selector);
    format!(r#"(() => document.querySelector("{selector}") ? "true" : "false")()"#)
}

fn selector_click_js(selector: &str) -> String {
    let selector = escape_js_string(selector);
    format!(
        r#"(() => {{
            const el = document.querySelector("{selector}");
            if (!el) throw new Error("selector not found");
            el.click();
            return "true";
        }})()"#
    )
}

fn selector_fill_js(selector: &str, text: &str) -> String {
    let selector = escape_js_string(selector);
    let text = escape_js_string(text);
    format!(
        r#"(() => {{
            const el = document.querySelector("{selector}");
            if (!el) throw new Error("selector not found");
            if ("value" in el) {{
                el.value = "{text}";
            }} else {{
                el.textContent = "{text}";
            }}
            el.dispatchEvent(new Event("input", {{ bubbles: true }}));
            el.dispatchEvent(new Event("change", {{ bubbles: true }}));
            return "true";
        }})()"#
    )
}

fn gemini_mode_placeholders(mode: GeminiMode) -> &'static [&'static str] {
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

fn should_skip_gemini_response(trimmed: &str, prompt: &str) -> bool {
    trimmed.is_empty() || trimmed == prompt.trim()
}

fn should_skip_chatgpt_response(trimmed: &str, prompt: &str) -> bool {
    trimmed.is_empty() || trimmed == prompt.trim()
}

fn gemini_mode_slug(mode: GeminiMode) -> &'static str {
    match mode {
        GeminiMode::Image => "image",
        GeminiMode::DeepResearch => "deep-research",
        GeminiMode::Video => "video",
        GeminiMode::Music => "music",
    }
}

fn gemini_mode_url(mode: GeminiMode) -> &'static str {
    match mode {
        GeminiMode::Image => "https://gemini.google.com/image",
        GeminiMode::DeepResearch => "https://gemini.google.com/deepresearch",
        GeminiMode::Video => "https://gemini.google.com/veo",
        GeminiMode::Music => "https://gemini.google.com/music",
    }
}

fn build_gemini_go_home_js() -> String {
    r#"(function() {
        window.location.href = "https://gemini.google.com/app";
        return "true";
    })()"#
        .to_string()
}

fn build_chatgpt_go_home_js() -> String {
    r#"(function() {
        window.location.href = "https://chatgpt.com/";
        return "true";
    })()"#
        .to_string()
}

pub fn ensure_gemini_home(profile_filter: Option<&str>) -> Result<(), MacosError> {
    let _ = execute_js_for_profile(
        &build_gemini_go_home_js(),
        profile_filter,
        "safari_gemini_go_home",
    )?;
    thread::sleep(Duration::from_millis(2500));
    Ok(())
}

pub fn ensure_chatgpt_home(profile_filter: Option<&str>) -> Result<(), MacosError> {
    let _ = execute_js_for_profile(
        &build_chatgpt_go_home_js(),
        profile_filter,
        "safari_chatgpt_go_home",
    )?;
    thread::sleep(Duration::from_millis(2500));
    Ok(())
}

fn build_gemini_placeholder_read_js() -> String {
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

fn gemini_response_extract_js() -> String {
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
fn build_gemini_fill_input_js(prompt: &str) -> String {
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

fn build_chatgpt_fill_input_js(prompt: &str) -> String {
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

fn wait_and_click_send(profile_filter: Option<&str>) -> Result<(), MacosError> {
    let js = &build_gemini_click_send_js();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
        let result = execute_js_for_profile(js, profile_filter, "safari_gemini_wait_send")?;
        if result.trim() == "true" {
            return Ok(());
        }
    }
    Err(MacosError::Other(
        "send button not found or disabled after 5s".to_string(),
    ))
}

fn build_gemini_click_send_js() -> String {
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

fn build_chatgpt_click_send_js() -> String {
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

fn wait_and_click_chatgpt_send(profile_filter: Option<&str>) -> Result<(), MacosError> {
    let js = &build_chatgpt_click_send_js();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
        let result = execute_js_for_profile(js, profile_filter, "safari_chatgpt_wait_send")?;
        if result.trim() == "true" {
            return Ok(());
        }
    }
    Err(MacosError::Other(
        "send button not found or disabled after 5s".to_string(),
    ))
}

fn gemini_deep_research_poll_js() -> String {
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

fn chatgpt_response_extract_js() -> String {
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

fn chatgpt_image_list_js() -> String {
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

fn poll_chatgpt_images(
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
        thread::sleep(Duration::from_secs(1));
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

fn chatgpt_image_extract_js(url: &str) -> String {
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

fn chatgpt_image_result_is_ready(result: &SafariAiImageResult) -> bool {
    result.status == "complete"
        && !result.images.is_empty()
        && result.images.iter().all(|image| image.loaded)
}

fn chatgpt_should_return_empty_complete_result(
    result: &SafariAiImageResult,
    elapsed: Duration,
    empty_complete_grace_seconds: u64,
) -> bool {
    result.status == "complete"
        && result.images.is_empty()
        && elapsed >= Duration::from_secs(empty_complete_grace_seconds)
}

fn parse_chatgpt_image_payload(payload: &str) -> Result<SafariAiImageResult, MacosError> {
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

fn get_gemini_conversation_url(profile_filter: Option<&str>) -> Result<String, MacosError> {
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

pub fn gemini_list_conversations(
    profile_filter: Option<&str>,
) -> Result<Vec<SafariConversation>, MacosError> {
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
}

fn chatgpt_list_conversations_js() -> String {
    r#"(() => {
        const nav = document.querySelector('nav[aria-label="聊天歷程紀錄"]');
        if (!nav) return JSON.stringify([]);
        const recentButton = Array.from(nav.querySelectorAll("button"))
          .find((button) => (button.innerText || button.textContent || "").trim() === "最近");
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

pub fn chatgpt_list_conversations(
    profile_filter: Option<&str>,
) -> Result<Vec<SafariConversation>, MacosError> {
    let js = chatgpt_list_conversations_js();
    let deadline = Instant::now() + Duration::from_secs(10);

    while Instant::now() < deadline {
        let raw = execute_js_for_profile(&js, profile_filter, "safari_chatgpt_list_conversations")?;
        let items: Vec<SafariConversation> = serde_json::from_str(&raw)
            .map_err(|e| MacosError::Other(format!("failed to parse conversations: {e}")))?;
        if !items.is_empty() {
            return Ok(items);
        }
        thread::sleep(Duration::from_millis(500));
    }

    Ok(Vec::new())
}

pub fn gemini_read_conversation(
    url: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiResponseResult, MacosError> {
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
    let value: serde_json::Value = serde_json::from_str(&raw)
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
}

pub fn gemini_save_images(
    conversation_url: &str,
    output_dir: &str,
    profile_filter: Option<&str>,
) -> Result<Vec<String>, MacosError> {
    let nav_js = format!(
        r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
        url = escape_js_string(conversation_url),
    );
    let _ = execute_js_for_profile(&nav_js, profile_filter, "safari_gemini_save_img_navigate")?;
    thread::sleep(Duration::from_millis(5000));

    let count_js = r#"(() => {
        const imgs = document.querySelectorAll('img[alt*="AI"], img[alt*="生成"]');
        return String(imgs.length);
    })()"#;
    let count: usize = execute_js_for_profile(count_js, profile_filter, "safari_gemini_img_count")?
        .trim()
        .parse()
        .unwrap_or(0);

    if count == 0 {
        return Ok(Vec::new());
    }

    let out_path = std::path::Path::new(output_dir);
    if !out_path.exists() {
        std::fs::create_dir_all(out_path)
            .map_err(|e| MacosError::Other(format!("failed to create output dir: {e}")))?;
    }

    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    let mut saved = Vec::new();

    for i in 0..count {
        let extract_js = format!(
            r#"(() => {{
                const imgs = document.querySelectorAll('img[alt*="AI"], img[alt*="生成"]');
                const img = imgs[{i}];
                if (!img) return "";
                const canvas = document.createElement("canvas");
                canvas.width = img.naturalWidth || img.width;
                canvas.height = img.naturalHeight || img.height;
                canvas.getContext("2d").drawImage(img, 0, 0);
                const dataUrl = canvas.toDataURL("image/png");
                const idx = dataUrl.indexOf(",");
                return idx >= 0 ? dataUrl.substring(idx + 1) : "";
            }})()"#,
        );
        let b64 = execute_js_for_profile(&extract_js, profile_filter, "safari_gemini_img_extract")?;
        let b64 = b64.trim();
        if b64.is_empty() {
            continue;
        }

        let bytes = engine
            .decode(b64)
            .map_err(|e| MacosError::Other(format!("base64 decode failed for image {i}: {e}")))?;

        let filename = format!("gemini_image_{i}.png");
        let filepath = out_path.join(&filename);
        std::fs::write(&filepath, &bytes).map_err(|e| {
            MacosError::Other(format!("failed to write {}: {e}", filepath.display()))
        })?;
        saved.push(filepath.to_string_lossy().into_owned());
    }

    Ok(saved)
}

pub fn gemini_save_media(
    conversation_url: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiResponseResult, MacosError> {
    let nav_js = format!(
        r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
        url = escape_js_string(conversation_url),
    );
    let _ = execute_js_for_profile(&nav_js, profile_filter, "safari_gemini_media_navigate")?;
    thread::sleep(Duration::from_millis(5000));

    let js = r#"(() => {
        const video = document.querySelector("video");
        if (!video) return JSON.stringify({ status: "error", response: "no media found" });
        const src = video.src || video.currentSrc || "";
        if (!src) return JSON.stringify({ status: "error", response: "no media source" });
        const a = document.createElement("a");
        a.href = src;
        a.download = "gemini_media.mp4";
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        return JSON.stringify({ status: "downloading", response: src });
    })()"#;
    let raw = execute_js_for_profile(js, profile_filter, "safari_gemini_media_download")?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| MacosError::Other(format!("failed to parse media result: {e}")))?;
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
}

pub fn threads_extract_feed(
    profile_filter: Option<&str>,
) -> Result<Vec<SocialFeedPost>, MacosError> {
    // Auto-focus to Threads tab
    let _ = focus_tab("threads.com", profile_filter);

    let js = r#"(() => {
        const authorLinks = [...document.querySelectorAll('a[href^="/@"]')];
        const seen = new Set();
        const posts = [];

        for (const link of authorLinks) {
            const href = link.getAttribute("href") || "";
            if (href.split("/").length > 3) continue;

            let container = link;
            for (let i = 0; i < 12; i++) {
                if (!container.parentElement) break;
                container = container.parentElement;
                if ((container.innerText || "").length > 30 && container.querySelector("time")) break;
            }

            const key = container.innerText.substring(0, 80);
            if (seen.has(key)) continue;
            seen.add(key);

            const author = href.replace("/", "");
            const timeEl = container.querySelector("time");
            const time = timeEl ? (timeEl.getAttribute("datetime") || timeEl.innerText || "") : "";
            const contentSpans = [...container.querySelectorAll('span[dir="auto"], div[dir="auto"]')];
            let content = "";
            for (const span of contentSpans) {
                const t = (span.innerText || "").trim();
                if (t.length > 20) { content = t; break; }
            }
            if (content) {
                posts.push({ author, time: time || null, content: content.substring(0, 500) });
            }
        }
        return JSON.stringify(posts);
    })()"#;
    let raw = execute_js_for_profile(js, profile_filter, "safari_threads_feed")?;
    let posts: Vec<SocialFeedPost> = serde_json::from_str(&raw)
        .map_err(|e| MacosError::Other(format!("failed to parse threads feed: {e}")))?;
    Ok(posts)
}

pub fn x_extract_feed(profile_filter: Option<&str>) -> Result<Vec<SocialFeedPost>, MacosError> {
    // Auto-focus to X tab
    let _ = focus_tab("x.com", profile_filter);

    let js = r#"(() => {
        const tweets = document.querySelectorAll('article[data-testid="tweet"]');
        const posts = [];
        const seen = new Set();

        for (const tweet of tweets) {
            const timeEl = tweet.querySelector("time");
            const time = timeEl ? (timeEl.getAttribute("datetime") || "") : "";
            const tweetText = tweet.querySelector('div[data-testid="tweetText"]');
            const content = tweetText ? (tweetText.innerText || "").trim() : "";

            const key = content.substring(0, 50);
            if (seen.has(key) || !content) continue;
            seen.add(key);

            const userCell = tweet.querySelector('div[data-testid="User-Name"]');
            const userText = userCell ? (userCell.innerText || "") : "";
            const author = userText.split("\n")[0] || "";
            const handle = (userText.match(/@\w+/) || [""])[0];

            const groups = tweet.querySelectorAll('div[role="group"] button');
            const metrics = [...groups]
                .map(b => (b.getAttribute("aria-label") || "").trim())
                .filter(Boolean);

            posts.push({
                author,
                handle: handle || null,
                time: time || null,
                content: content.substring(0, 500),
                metrics
            });
        }
        return JSON.stringify(posts);
    })()"#;
    let raw = execute_js_for_profile(js, profile_filter, "safari_x_feed")?;
    let posts: Vec<SocialFeedPost> = serde_json::from_str(&raw)
        .map_err(|e| MacosError::Other(format!("failed to parse x feed: {e}")))?;
    Ok(posts)
}

pub fn capture(since: DateTime<Utc>) -> Result<Vec<Cue>, MacosError> {
    let db_path = history_db_path();

    if !db_path.exists() {
        return Err(MacosError::PermissionDenied(
            db_path.to_string_lossy().into_owned(),
        ));
    }

    let conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        if e.to_string().contains("unable to open") {
            MacosError::PermissionDenied(db_path.to_string_lossy().into_owned())
        } else {
            MacosError::Sqlite(e)
        }
    })?;

    let since_ts = to_core_data_timestamp(since);

    let mut stmt = conn.prepare(
        "SELECT v.visit_time, v.title, i.url \
         FROM history_visits v \
         JOIN history_items i ON v.history_item = i.id \
         WHERE v.visit_time > ?1 \
         ORDER BY v.visit_time DESC",
    )?;

    let cues = stmt
        .query_map([since_ts], |row| {
            let visit_time: f64 = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let url: String = row.get(2)?;
            Ok((visit_time, title, url))
        })?
        .filter_map(|r| r.ok())
        .map(|(visit_time, title, url)| Cue {
            source: CueSource::Safari,
            timestamp: from_core_data_timestamp(visit_time),
            content: title.clone().unwrap_or_default(),
            url: Some(url),
            title,
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        })
        .collect();

    Ok(cues)
}

pub fn tabs(profile_filter: Option<&str>) -> Result<Vec<SafariTab>, MacosError> {
    let stdout = run_capture(&build_tabs_script(), "safari_tabs")?;
    let mut tabs = parse_tabs_output(&stdout);
    if let Some(profile) = profile_filter {
        tabs.retain(|tab| tab.profile.as_deref() == Some(profile));
    }
    Ok(tabs)
}

pub fn active(profile_filter: Option<&str>) -> Result<Option<SafariTab>, MacosError> {
    let stdout = run_capture(&build_active_tab_script(profile_filter), "safari_active")?;
    Ok(parse_tab_line(stdout.trim()).map(|mut tab| {
        tab.profile = extract_profile(&tab.window_name, &tab.title);
        tab
    }))
}

pub fn open(url: &str, profile_filter: Option<&str>) -> Result<Option<SafariTab>, MacosError> {
    let stdout = run_capture(&build_open_script(url, profile_filter), "safari_open")?;
    Ok(parse_tab_line(stdout.trim()).map(|mut tab| {
        tab.profile = extract_profile(&tab.window_name, &tab.title);
        tab
    }))
}

/// Focus a specific tab by index or by matching URL/title substring.
/// This sets the matched tab as the current tab so subsequent operations target it.
pub fn focus_tab(
    tab_selector: &str,
    profile_filter: Option<&str>,
) -> Result<SafariTab, MacosError> {
    let all_tabs = tabs(profile_filter)?;
    if all_tabs.is_empty() {
        return Err(MacosError::Other("no Safari tabs found".to_string()));
    }

    // Try parsing as index first (position in the flat tab list)
    let matched = if let Ok(index) = tab_selector.parse::<usize>() {
        all_tabs.into_iter().nth(index)
    } else {
        // Match by URL or title substring
        let query = tab_selector.to_lowercase();
        all_tabs.into_iter().find(|t| {
            t.url.to_lowercase().contains(&query) || t.title.to_lowercase().contains(&query)
        })
    };

    let tab =
        matched.ok_or_else(|| MacosError::Other(format!("no tab matching '{tab_selector}'")))?;

    // Set as current tab
    let script = format!(
        r#"
        tell application "Safari"
            repeat with w in every window
                if (id of w) is {window_id} then
                    set current tab of w to tab {one_based} of w
                    set index of w to 1
                    return "true"
                end if
            end repeat
            return "false"
        end tell
        "#,
        window_id = tab.window_id,
        one_based = tab.index + 1,
    );
    let result = run_capture(&script, "safari_focus_tab")?;
    if result.trim() != "true" {
        return Err(MacosError::Other(format!(
            "failed to focus tab '{}'",
            tab_selector
        )));
    }

    Ok(tab)
}

pub fn close(index: Option<usize>) -> Result<SafariCloseResult, MacosError> {
    let stdout = run_capture(&build_close_script(index), "safari_close")?;
    Ok(SafariCloseResult {
        closed: stdout.trim() == "true",
        index,
    })
}

pub fn close_tabs(
    profile_filter: Option<&str>,
    url_pattern: Option<&str>,
) -> Result<usize, MacosError> {
    let all_tabs = tabs(profile_filter)?;
    let to_close: Vec<&SafariTab> = all_tabs
        .iter()
        .filter(|tab| match url_pattern {
            Some(pattern) => tab.url.contains(pattern),
            None => true,
        })
        .collect();

    // Close from last to first to avoid index shifting
    let mut closed = 0;
    for tab in to_close.iter().rev() {
        let script = format!(
            r#"
            tell application "Safari"
                repeat with w in every window
                    if (id of w) is {window_id} then
                        set tabIdx to {tab_index} + 1
                        if tabIdx ≤ (count of tabs of w) then
                            close tab tabIdx of w
                        end if
                        exit repeat
                    end if
                end repeat
            end tell
            "#,
            window_id = tab.window_id,
            tab_index = tab.index,
        );
        if run_capture(&script, "safari_close_tab").is_ok() {
            closed += 1;
        }
    }

    Ok(closed)
}

pub fn source(profile_filter: Option<&str>) -> Result<SafariSourceResult, MacosError> {
    let result = execute_js_for_profile(
        "document.documentElement.outerHTML",
        profile_filter,
        "safari_source",
    )?;
    Ok(SafariSourceResult { html: result })
}

pub fn read(
    selector: Option<&str>,
    profile_filter: Option<&str>,
) -> Result<SafariReadResult, MacosError> {
    let js = match selector {
        Some(selector) => selector_text_js(selector),
        None => "(document.body.innerText ?? \"\").trim()".to_string(),
    };
    let content = execute_js_for_profile(&js, profile_filter, "safari_read")?;
    Ok(SafariReadResult {
        selector: selector.map(ToOwned::to_owned),
        content,
    })
}

fn block_fingerprint(text: &str) -> String {
    text.chars().take(120).collect()
}

fn scroll_read_new_content_blocks(content: &str, seen: &[String]) -> Vec<String> {
    let seen_fps: Vec<String> = seen.iter().map(|s| block_fingerprint(s)).collect();
    let mut new_blocks = Vec::new();
    let mut new_fps = Vec::new();
    for block in content
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
    {
        let fp = block_fingerprint(block);
        if seen_fps.iter().any(|existing| *existing == fp)
            || new_fps.iter().any(|existing: &String| *existing == fp)
        {
            continue;
        }
        new_fps.push(fp);
        new_blocks.push(block.to_string());
    }
    new_blocks
}

fn scroll_read_snapshot_blocks(
    snapshot: &SafariScrollReadSnapshot,
    seen: &[String],
) -> Vec<String> {
    if !snapshot.blocks.is_empty() {
        let seen_fps: Vec<String> = seen.iter().map(|s| block_fingerprint(s)).collect();
        let mut new_blocks = Vec::new();
        let mut new_fps = Vec::new();
        for block in &snapshot.blocks {
            let trimmed = block.trim();
            if trimmed.is_empty() {
                continue;
            }
            let fp = block_fingerprint(trimmed);
            if seen_fps.iter().any(|existing| *existing == fp)
                || new_fps.iter().any(|existing: &String| *existing == fp)
            {
                continue;
            }
            new_fps.push(fp);
            new_blocks.push(trimmed.to_string());
        }
        return new_blocks;
    }

    scroll_read_new_content_blocks(&snapshot.content, seen)
}

fn scroll_read_detects_new_content(
    previous_text: &str,
    previous_count: usize,
    current_text: &str,
    current_count: usize,
) -> bool {
    current_count > previous_count || current_text.trim() != previous_text.trim()
}

pub fn exec(js_code: &str, profile_filter: Option<&str>) -> Result<SafariEvalResult, MacosError> {
    let result = execute_js_for_profile(js_code, profile_filter, "safari_exec")?;
    Ok(SafariEvalResult { result })
}

pub fn click(selector: &str) -> Result<SafariClickResult, MacosError> {
    let result = execute_js(&selector_click_js(selector), "safari_click")?;
    if result.trim() != "true" {
        return Err(MacosError::Other(result));
    }
    Ok(SafariClickResult {
        clicked: true,
        selector: selector.to_string(),
    })
}

pub fn fill(selector: &str, text: &str) -> Result<SafariFillResult, MacosError> {
    let result = execute_js(&selector_fill_js(selector, text), "safari_fill")?;
    if result.trim() != "true" {
        return Err(MacosError::Other(result));
    }
    Ok(SafariFillResult {
        filled: true,
        selector: selector.to_string(),
        text: text.to_string(),
    })
}

pub fn wait(selector: &str, timeout_seconds: u64) -> Result<SafariWaitResult, MacosError> {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    let js = selector_exists_js(selector);
    loop {
        let exists = execute_js(&js, "safari_wait")?;
        if exists.is_empty() {
            return Err(MacosError::Other(
                "no Safari window or active tab available".to_string(),
            ));
        }
        if exists.trim() == "true" {
            return Ok(SafariWaitResult {
                found: true,
                selector: selector.to_string(),
                timeout_seconds,
            });
        }
        if Instant::now() >= deadline {
            return Err(MacosError::Other(format!(
                "timeout waiting for selector: {selector}"
            )));
        }
        thread::sleep(Duration::from_millis(250));
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariScrollResult {
    pub scroll_x: i64,
    pub scroll_y: i64,
}

#[derive(Debug, serde::Deserialize, PartialEq, Eq)]
struct SafariScrollReadSnapshot {
    item_count: usize,
    content: String,
    #[serde(default)]
    blocks: Vec<String>,
}

pub fn scroll(
    direction: &str,
    amount: Option<i64>,
    profile_filter: Option<&str>,
) -> Result<SafariScrollResult, MacosError> {
    let pixels = amount.unwrap_or(500).unsigned_abs();
    let js = match direction {
        "down" => format!("(function(){{ window.scrollBy(0, {pixels}); return JSON.stringify({{ x: Math.round(window.scrollX), y: Math.round(window.scrollY) }}); }})()"),
        "up" => format!("(function(){{ window.scrollBy(0, -{pixels}); return JSON.stringify({{ x: Math.round(window.scrollX), y: Math.round(window.scrollY) }}); }})()"),
        "top" => "(function(){ window.scrollTo(0, 0); return JSON.stringify({ x: 0, y: 0 }); })()".to_string(),
        "bottom" => "(function(){ window.scrollTo(0, document.body.scrollHeight); return JSON.stringify({ x: Math.round(window.scrollX), y: Math.round(window.scrollY) }); })()".to_string(),
        _ => return Err(MacosError::Other(format!("unknown scroll direction: {direction}. Use: up, down, top, bottom"))),
    };
    let raw = execute_js_for_profile(&js, profile_filter, "safari_scroll")?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| MacosError::Other(format!("failed to parse scroll result: {e}")))?;
    Ok(SafariScrollResult {
        scroll_x: value.get("x").and_then(|v| v.as_i64()).unwrap_or(0),
        scroll_y: value.get("y").and_then(|v| v.as_i64()).unwrap_or(0),
    })
}

fn scroll_read_poll_js(selector: Option<&str>) -> String {
    let scope_expr = match selector {
        Some(selector) => format!(
            "document.querySelector(\"{}\")",
            escape_js_string(selector)
        ),
        None => "document.body".to_string(),
    };
    format!(
        r#"(() => {{
            const scope = {scope_expr};
            if (!scope) return JSON.stringify({{ item_count: 0, content: "", blocks: [] }});
            const text = (scope.innerText || scope.textContent || "").trim();
            const isVisible = (node) => {{
              const rect = node.getBoundingClientRect();
              return rect.bottom > 0 && rect.top < window.innerHeight;
            }};
            // Phase 1: semantic selectors (Reddit, etc.)
            let itemNodes = [
              ...scope.querySelectorAll("shreddit-post"),
              ...scope.querySelectorAll("[data-testid='post-container']"),
              ...scope.querySelectorAll("article"),
              ...scope.querySelectorAll("[role='article']")
            ];
            // Phase 2: heuristic fallback for CSR sites (Threads, X, etc.)
            if (itemNodes.length === 0) {{
              const MIN_TEXT = 50;
              const MAX_TEXT = 3000;
              itemNodes = [...scope.querySelectorAll("div")].filter(el => {{
                const t = (el.innerText || "").trim();
                if (t.length < MIN_TEXT || t.length > MAX_TEXT) return false;
                // Skip wrappers: if any direct child div holds >80% of this text, it's a wrapper
                const kids = el.querySelectorAll(":scope > div");
                for (const c of kids) {{
                  if ((c.innerText || "").trim().length > t.length * 0.8) return false;
                }}
                return true;
              }});
            }}
            const visibleItemNodes = itemNodes.filter(isVisible);
            const blocks = [];
            const seen = new Set();
            for (const node of visibleItemNodes) {{
              const block = (node.innerText || node.textContent || "").trim();
              if (!block || block.length < 20) continue;
              const key = block.slice(0, 120);
              if (seen.has(key)) continue;
              seen.add(key);
              blocks.push(block);
            }}
            const itemCount = visibleItemNodes.length
              || scope.querySelectorAll("li").length
              || scope.children.length
              || 0;
            return JSON.stringify({{
              item_count: itemCount,
              content: text,
              blocks
            }});
        }})()"#
    )
}

fn parse_scroll_read_snapshot(payload: &str) -> Result<SafariScrollReadSnapshot, MacosError> {
    serde_json::from_str(payload)
        .map_err(|error| MacosError::Other(format!("invalid scroll/read payload: {error}: {payload}")))
}

fn poll_scroll_read_snapshot(
    selector: Option<&str>,
    previous: &SafariScrollReadSnapshot,
    profile_filter: Option<&str>,
    timeout: Duration,
) -> Result<SafariScrollReadSnapshot, MacosError> {
    let deadline = Instant::now() + timeout;
    let js = scroll_read_poll_js(selector);

    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(500));
        let payload = execute_js_for_profile(&js, profile_filter, "safari_scroll_read_poll")?;
        let snapshot = parse_scroll_read_snapshot(&payload)?;
        if scroll_read_detects_new_content(
            &previous.content,
            previous.item_count,
            &snapshot.content,
            snapshot.item_count,
        ) {
            return Ok(snapshot);
        }
    }

    let payload = execute_js_for_profile(&js, profile_filter, "safari_scroll_read_poll")?;
    parse_scroll_read_snapshot(&payload)
}

pub fn scroll_and_read(
    times: u64,
    amount: Option<i64>,
    selector: Option<&str>,
    profile_filter: Option<&str>,
) -> Result<SafariScrollReadResult, MacosError> {
    let js = scroll_read_poll_js(selector);
    let payload = execute_js_for_profile(&js, profile_filter, "safari_scroll_read_initial")?;
    let mut snapshot = parse_scroll_read_snapshot(&payload)?;
    let mut seen = scroll_read_snapshot_blocks(&snapshot, &[]);
    let mut chunks = Vec::new();

    for iteration in 1..=times {
        scroll("down", amount, profile_filter)?;
        snapshot = poll_scroll_read_snapshot(
            selector,
            &snapshot,
            profile_filter,
            Duration::from_secs(5),
        )?;
        let new_blocks = scroll_read_snapshot_blocks(&snapshot, &seen);
        if new_blocks.is_empty() {
            continue;
        }
        seen.extend(new_blocks.iter().cloned());
        chunks.push(SafariScrollReadChunk {
            iteration,
            content: new_blocks.join("\n\n"),
        });
    }

    Ok(SafariScrollReadResult {
        selector: selector.map(ToOwned::to_owned),
        times,
        chunks,
    })
}

fn parse_deep_research_payload(payload: &str) -> Result<SafariDeepResearchResult, MacosError> {
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

pub fn prepare_gemini_mode(
    mode: GeminiMode,
    profile_filter: Option<&str>,
) -> Result<SafariAiReadyResult, MacosError> {
    let url = gemini_mode_url(mode);
    let nav_js = format!(r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,);
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
}

pub fn start_gemini_deep_research(
    prompt: &str,
    auto_confirm: bool,
    profile_filter: Option<&str>,
) -> Result<SafariDeepResearchResult, MacosError> {
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

    wait_and_click_send(profile_filter)?;

    // Capture conversation URL after submission
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
        wait_and_click_send(profile_filter)?;
        // Wait for page to transition from plan_ready to running state
        thread::sleep(Duration::from_secs(3));
        let mut final_result = poll_gemini_deep_research(900, profile_filter)?;
        final_result.conversation_url = conv_url;
        Ok(final_result)
    } else {
        Ok(result)
    }
}

pub fn poll_gemini_deep_research(
    timeout_seconds: u64,
    profile_filter: Option<&str>,
) -> Result<SafariDeepResearchResult, MacosError> {
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
}

pub fn send_gemini_prompt(
    prompt: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiResponseResult, MacosError> {
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

    wait_and_click_send(profile_filter)?;

    let mut last_text = String::new();
    let mut stable_count = 0;
    let deadline = Instant::now() + Duration::from_secs(60);
    let response_js = gemini_response_extract_js();

    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(1));
        let text = execute_js_for_profile(&response_js, profile_filter, "safari_gemini_response")?;
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
}

pub fn send_chatgpt_prompt(
    prompt: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiResponseResult, MacosError> {
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

    wait_and_click_chatgpt_send(profile_filter)?;

    let deadline = Instant::now() + Duration::from_secs(120);
    let response_js = chatgpt_response_extract_js();
    let mut last_response = String::new();
    let mut last_conversation_url: Option<String> = None;

    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(750));
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
}

pub fn send_chatgpt_image_prompt(
    prompt: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiImageResult, MacosError> {
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

    wait_and_click_chatgpt_send(profile_filter)?;
    poll_chatgpt_images(180, 3, profile_filter)
}

pub fn chatgpt_save_images(
    conversation_url: &str,
    output_dir: &str,
    profile_filter: Option<&str>,
) -> Result<Vec<String>, MacosError> {
    let nav_js = format!(
        r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
        url = escape_js_string(conversation_url),
    );
    let _ = execute_js_for_profile(&nav_js, profile_filter, "safari_chatgpt_save_img_navigate")?;
    let result = poll_chatgpt_images(30, 8, profile_filter)?;
    if result.images.is_empty() {
        return Ok(Vec::new());
    }

    let out_path = std::path::Path::new(output_dir);
    if !out_path.exists() {
        std::fs::create_dir_all(out_path)
            .map_err(|e| MacosError::Other(format!("failed to create output dir: {e}")))?;
    }

    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    let mut saved = Vec::new();
    let filename_prefix = result
        .conversation_url
        .as_deref()
        .and_then(|url| url.rsplit('/').next())
        .filter(|segment| !segment.is_empty())
        .unwrap_or("chatgpt");

    for (i, image) in result.images.iter().enumerate() {
        let b64 = execute_js_for_profile(
            &chatgpt_image_extract_js(&image.url),
            profile_filter,
            "safari_chatgpt_img_extract",
        )?;
        let b64 = b64.trim();
        if b64.is_empty() {
            continue;
        }

        let bytes = engine
            .decode(b64)
            .map_err(|e| MacosError::Other(format!("base64 decode failed for image {i}: {e}")))?;

        let filename = format!("{filename_prefix}_image_{i}.png");
        let filepath = out_path.join(&filename);
        std::fs::write(&filepath, &bytes).map_err(|e| {
            MacosError::Other(format!("failed to write {}: {e}", filepath.display()))
        })?;
        saved.push(filepath.to_string_lossy().into_owned());
    }

    Ok(saved)
}

fn execute_js(js_code: &str, context: &str) -> Result<String, MacosError> {
    execute_js_for_profile(js_code, None, context)
}

fn execute_js_for_profile(
    js_code: &str,
    profile_filter: Option<&str>,
    context: &str,
) -> Result<String, MacosError> {
    let stdout = run_capture(
        &build_exec_script_for_profile(js_code, profile_filter),
        context,
    )?;
    Ok(decode_field(stdout.trim()))
}

#[cfg(test)]
mod tests {
    use super::{
        SafariAiImage, SafariAiImageResult, SafariScrollReadSnapshot, TAB_SEPARATOR, build_active_tab_script,
        build_chatgpt_click_send_js, build_chatgpt_fill_input_js, build_chatgpt_go_home_js,
        build_close_script, build_exec_script, build_gemini_fill_input_js, build_gemini_go_home_js,
        build_open_script, build_tab_return_block, build_tabs_script, chatgpt_image_extract_js,
        chatgpt_image_list_js, chatgpt_image_result_is_ready, chatgpt_response_extract_js,
        chatgpt_list_conversations_js,
        chatgpt_should_return_empty_complete_result, extract_profile, gemini_deep_research_poll_js,
        gemini_response_extract_js, parse_chatgpt_image_payload, parse_deep_research_payload,
        parse_tab_line, parse_tabs_output, scroll_read_detects_new_content,
        scroll_read_poll_js, scroll_read_new_content_blocks, scroll_read_snapshot_blocks,
        selector_click_js, selector_fill_js, selector_text_js,
        should_skip_chatgpt_response, should_skip_gemini_response,
    };
    use std::time::Duration;

    #[test]
    fn extract_profile_from_window_name() {
        let profile = extract_profile("Ryugu — Google Gemini", "Google Gemini");

        assert_eq!(profile.as_deref(), Some("Ryugu"));
    }

    #[test]
    fn parse_tab_line_decodes_fields() {
        let line = "61998<<<FIELD_SEP>>>Ryugu — Google\\tGemini<<<FIELD_SEP>>>0<<<FIELD_SEP>>>Google\\tGemini<<<FIELD_SEP>>>https://gemini.google.com/app<<<FIELD_SEP>>>true";

        let tab = parse_tab_line(line).expect("tab");

        assert_eq!(tab.window_id, 61998);
        assert_eq!(tab.window_name, "Ryugu — Google\tGemini");
        assert_eq!(tab.profile, None);
        assert_eq!(tab.index, 0);
        assert_eq!(tab.title, "Google\tGemini");
        assert_eq!(tab.url, "https://gemini.google.com/app");
        assert!(tab.active);
    }

    #[test]
    fn parse_tabs_output_keeps_multiple_tabs() {
        let raw = concat!(
            "1<<<FIELD_SEP>>>Work — Mail<<<FIELD_SEP>>>0<<<FIELD_SEP>>>Mail<<<FIELD_SEP>>>https://mail.google.com<<<FIELD_SEP>>>true---TAB_SEP---",
            "1<<<FIELD_SEP>>>Work — Mail<<<FIELD_SEP>>>1<<<FIELD_SEP>>>Docs<<<FIELD_SEP>>>https://docs.google.com<<<FIELD_SEP>>>false---TAB_SEP---"
        );

        let tabs = parse_tabs_output(raw);

        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].title, "Mail");
        assert_eq!(tabs[1].title, "Docs");
        assert_eq!(tabs[0].profile.as_deref(), Some("Work"));
        assert_eq!(tabs[1].profile.as_deref(), Some("Work"));
    }

    #[test]
    fn safari_script_escapes_record_separator() {
        let script = build_tabs_script();

        assert!(script.contains(TAB_SEPARATOR));
        assert!(script.contains("\\s"));
        assert!(script.contains("<<<FIELD_SEP>>>"));
    }

    #[test]
    fn build_open_script_creates_new_tab() {
        let script = build_open_script("https://example.com", None);

        assert!(script.contains("make new tab at end of tabs of w"));
        assert!(script.contains("https://example.com"));
    }

    #[test]
    fn build_close_script_targets_requested_index() {
        let script = build_close_script(Some(2));

        assert!(script.contains("set t to tab 3 of w"));
    }

    #[test]
    fn build_active_tab_script_targets_front_window() {
        let script = build_active_tab_script(None);

        assert!(script.contains("set w to front window"));
        assert!(script.contains("set t to current tab of w"));
    }

    #[test]
    fn build_tab_return_block_coerces_numeric_fields_to_text() {
        let script = build_tab_return_block("t", "true");

        assert!(script.contains("(winId as text)"));
        assert!(script.contains("(tabIndex as text)"));
        assert!(script.contains("<<<FIELD_SEP>>>"));
    }

    #[test]
    fn build_exec_script_supports_multiline_js() {
        let script = build_exec_script("const x = 1;\nx + 1;");

        assert!(script.contains("set jsCode to"));
        assert!(script.contains("set rawResult to missing value"));
        assert!(script.contains("try"));
        assert!(script.contains("do JavaScript jsCode"));
        assert!(script.contains("on error errMsg"));
        assert!(script.contains("& linefeed &"));
        assert!(script.contains("if rawResult is missing value then"));
    }

    #[test]
    fn selector_js_builders_include_selector_and_text() {
        assert!(selector_text_js(".item").contains("querySelector(\".item\")"));
        assert!(selector_click_js("#submit").contains("querySelector(\"#submit\")"));
        let fill = selector_fill_js("input[name=q]", "hello");
        assert!(fill.contains("input[name=q]"));
        assert!(fill.contains("hello"));
    }

    #[test]
    fn gemini_fill_input_script_targets_editor() {
        let script = build_gemini_fill_input_js("hello world");

        assert!(script.contains(".ql-editor"));
        assert!(script.contains("hello world"));
        assert!(script.contains("execCommand"));
    }

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
    fn gemini_go_home_script_targets_root_app() {
        let script = build_gemini_go_home_js();

        assert!(script.contains("gemini.google.com/app"));
    }

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

    #[test]
    fn gemini_response_extract_script_targets_latest_response() {
        let script = gemini_response_extract_js();

        assert!(script.contains(".model-response-text"));
        assert!(script.contains("querySelectorAll"));
        assert!(script.contains("elements[elements.length - 1]"));
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

        assert!(script.contains("nav[aria-label=\"聊天歷程紀錄\"]"));
        assert!(script.contains("a[href^=\"/c/\"]"));
        assert!(script.contains("https://chatgpt.com"));
        assert!(script.contains("最近"));
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

    #[test]
    fn scroll_read_dedup_keeps_only_new_blocks() {
        let seen = vec!["alpha".to_string(), "beta".to_string()];
        let content = "alpha\n\nbeta\n\ngamma\n\ndelta\n\ngamma";

        let blocks = scroll_read_new_content_blocks(content, &seen);

        assert_eq!(blocks, vec!["gamma".to_string(), "delta".to_string()]);
    }

    #[test]
    fn scroll_read_change_detection_uses_count_or_text() {
        assert!(scroll_read_detects_new_content("same", 2, "same", 3));
        assert!(scroll_read_detects_new_content("before", 2, "after", 2));
        assert!(!scroll_read_detects_new_content("same", 2, "same", 2));
    }

    #[test]
    fn scroll_read_snapshot_blocks_prefers_structured_blocks() {
        let snapshot = SafariScrollReadSnapshot {
            item_count: 2,
            content: "alpha\n\nbeta\n\ngamma".to_string(),
            blocks: vec!["alpha".to_string(), "gamma".to_string()],
        };

        let blocks = scroll_read_snapshot_blocks(&snapshot, &["alpha".to_string()]);

        assert_eq!(blocks, vec!["gamma".to_string()]);
    }

    #[test]
    fn scroll_read_poll_script_exposes_count_and_text() {
        let script = scroll_read_poll_js(Some("main"));

        assert!(script.contains("item_count"));
        assert!(script.contains("content"));
        assert!(script.contains("blocks"));
        assert!(script.contains("querySelectorAll(\"article\")"));
        assert!(script.contains("document.querySelector(\"main\")"));
    }

    #[test]
    fn scroll_read_poll_script_uses_visible_items() {
        let script = scroll_read_poll_js(None);

        assert!(script.contains("getBoundingClientRect"));
        assert!(script.contains("window.innerHeight"));
        assert!(script.contains("rect.bottom > 0"));
    }

    #[test]
    fn scroll_read_poll_script_has_heuristic_fallback() {
        let script = scroll_read_poll_js(None);

        // Should fall back to div heuristic when no semantic selectors match
        assert!(script.contains("itemNodes.length === 0"));
        assert!(script.contains("querySelectorAll(\"div\")"));
    }

    #[test]
    fn scroll_read_fingerprint_dedup_handles_suffix_changes() {
        // CSR sites may re-render with slightly different suffixes.
        // Fingerprint uses first 120 chars, so blocks sharing a long prefix get deduped.
        let long_prefix = "a]".repeat(61); // 122 chars — first 120 identical
        let seen = vec![format!("{long_prefix}ORIGINAL_SUFFIX")];
        let content = format!("{long_prefix}UPDATED_SUFFIX\n\ngenuinely new block");

        let blocks = scroll_read_new_content_blocks(&content, &seen);

        // First block matches by prefix fingerprint, should be deduped
        assert_eq!(blocks, vec!["genuinely new block".to_string()]);
    }

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
}
