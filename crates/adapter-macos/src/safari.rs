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

fn build_open_script(url: &str) -> String {
    let escaped_url = escape(url);
    let tab_return = build_tab_return_block("t", "true");
    format!(
        r#"
        {prelude}
        tell application "Safari"
            if (count of windows) is 0 then
                make new document with properties {{URL:"{escaped_url}"}}
                set w to front window
            else
                set w to front window
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
        GeminiMode::Music => &["描述音樂", "描述要創作的音樂", "Describe the music", "Describe the music you"],
    }
}

fn should_skip_gemini_response(trimmed: &str, prompt: &str) -> bool {
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

fn ensure_gemini_home(profile_filter: Option<&str>) -> Result<(), MacosError> {
    let _ = execute_js_for_profile(
        &build_gemini_go_home_js(),
        profile_filter,
        "safari_gemini_go_home",
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
    Err(MacosError::Other("send button not found or disabled after 5s".to_string()))
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

pub fn gemini_list_conversations(profile_filter: Option<&str>) -> Result<Vec<SafariConversation>, MacosError> {
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
    let status = value.get("status").and_then(|v| v.as_str()).unwrap_or("error");
    let response = value.get("response").and_then(|v| v.as_str()).unwrap_or("");
    Ok(SafariAiResponseResult {
        provider: "gemini".to_string(),
        status: status.to_string(),
        response: response.to_string(),
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

        let bytes = engine.decode(b64).map_err(|e| {
            MacosError::Other(format!("base64 decode failed for image {i}: {e}"))
        })?;

        let filename = format!("gemini_image_{i}.png");
        let filepath = out_path.join(&filename);
        std::fs::write(&filepath, &bytes)
            .map_err(|e| MacosError::Other(format!("failed to write {}: {e}", filepath.display())))?;
        saved.push(filepath.to_string_lossy().into_owned());
    }

    Ok(saved)
}

pub fn gemini_get_video_url(
    conversation_url: &str,
    profile_filter: Option<&str>,
) -> Result<Option<String>, MacosError> {
    let nav_js = format!(
        r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
        url = escape_js_string(conversation_url),
    );
    let _ = execute_js_for_profile(&nav_js, profile_filter, "safari_gemini_video_navigate")?;
    thread::sleep(Duration::from_millis(3000));

    let js = r#"(() => {
        const video = document.querySelector("video");
        if (!video) return "";
        return video.src || video.currentSrc || "";
    })()"#;
    let url = execute_js_for_profile(js, profile_filter, "safari_gemini_video_url")?;
    let url = url.trim();
    if url.is_empty() {
        Ok(None)
    } else {
        Ok(Some(url.to_string()))
    }
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

pub fn open(url: &str) -> Result<Option<SafariTab>, MacosError> {
    let stdout = run_capture(&build_open_script(url), "safari_open")?;
    Ok(parse_tab_line(stdout.trim()).map(|mut tab| {
        tab.profile = extract_profile(&tab.window_name, &tab.title);
        tab
    }))
}

pub fn close(index: Option<usize>) -> Result<SafariCloseResult, MacosError> {
    let stdout = run_capture(&build_close_script(index), "safari_close")?;
    Ok(SafariCloseResult {
        closed: stdout.trim() == "true",
        index,
    })
}

pub fn source() -> Result<SafariSourceResult, MacosError> {
    let result = execute_js("document.documentElement.outerHTML", "safari_source")?;
    Ok(SafariSourceResult { html: result })
}

pub fn read(selector: Option<&str>) -> Result<SafariReadResult, MacosError> {
    let js = match selector {
        Some(selector) => selector_text_js(selector),
        None => "(document.body.innerText ?? \"\").trim()".to_string(),
    };
    let content = execute_js(&js, "safari_read")?;
    Ok(SafariReadResult {
        selector: selector.map(ToOwned::to_owned),
        content,
    })
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

fn parse_deep_research_payload(payload: &str) -> Result<SafariDeepResearchResult, MacosError> {
    let value: Value = serde_json::from_str(payload)
        .map_err(|error| MacosError::Other(format!("invalid deep research payload: {error}: {payload}")))?;
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
        plan: value.get("plan").and_then(Value::as_str).map(ToOwned::to_owned),
        response: value.get("response").and_then(Value::as_str).map(ToOwned::to_owned),
        actions,
    })
}

pub fn prepare_gemini_mode(
    mode: GeminiMode,
    profile_filter: Option<&str>,
) -> Result<SafariAiReadyResult, MacosError> {
    let url = gemini_mode_url(mode);
    let nav_js = format!(
        r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
    );
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
        return Err(MacosError::Other(format!("failed to fill deep research input: {filled}")));
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

    if auto_confirm {
        let filled = execute_js_for_profile(
            &build_gemini_fill_input_js("ok"),
            profile_filter,
            "safari_gemini_deep_research_confirm_fill",
        )?;
        if filled.trim() != "true" {
            return Err(MacosError::Other("failed to fill confirm text".to_string()));
        }
        wait_and_click_send(profile_filter)?;
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
        let payload = execute_js_for_profile(&js, profile_filter, "safari_gemini_deep_research_poll")?;
        let result = parse_deep_research_payload(&payload)?;
        match result.status.as_str() {
            "plan_ready" | "complete" => return Ok(result),
            "running" => {}
            _ => {
                return Err(MacosError::Other(format!(
                    "unknown deep research status: {}",
                    result.status
                )))
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
    ensure_gemini_home(profile_filter)?;

    let filled = execute_js_for_profile(
        &build_gemini_fill_input_js(prompt),
        profile_filter,
        "safari_gemini_prompt_fill",
    )?;
    if filled.trim() != "true" {
        return Err(MacosError::Other(format!("failed to fill Gemini input: {filled}")));
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
        });
    }

    Err(MacosError::Other(
        "timeout waiting for Gemini response".to_string(),
    ))
}

fn execute_js(js_code: &str, context: &str) -> Result<String, MacosError> {
    execute_js_for_profile(js_code, None, context)
}

fn execute_js_for_profile(
    js_code: &str,
    profile_filter: Option<&str>,
    context: &str,
) -> Result<String, MacosError> {
    let stdout = run_capture(&build_exec_script_for_profile(js_code, profile_filter), context)?;
    Ok(decode_field(stdout.trim()))
}

#[cfg(test)]
mod tests {
    use super::{
        TAB_SEPARATOR, build_active_tab_script, build_close_script,
        build_exec_script, build_gemini_fill_input_js, build_gemini_go_home_js,
        build_open_script,
        build_tab_return_block, build_tabs_script, extract_profile, gemini_deep_research_poll_js,
        gemini_response_extract_js, parse_deep_research_payload, parse_tab_line,
        parse_tabs_output, selector_click_js, selector_fill_js, selector_text_js,
        should_skip_gemini_response,
    };

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
        let script = build_open_script("https://example.com");

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
    fn parse_deep_research_payload_reads_plan_and_actions() {
        let payload = r#"{"status":"plan_ready","plan":"Outline","actions":["confirm","edit"]}"#;
        let result = parse_deep_research_payload(payload).expect("parse payload");

        assert_eq!(result.status, "plan_ready");
        assert_eq!(result.plan.as_deref(), Some("Outline"));
        assert_eq!(result.actions, vec!["confirm".to_string(), "edit".to_string()]);
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
    fn should_skip_gemini_response_trims_prompt_whitespace() {
        assert!(should_skip_gemini_response("hello", "  hello  "));
        assert!(!should_skip_gemini_response("world", "  hello  "));
    }
}
