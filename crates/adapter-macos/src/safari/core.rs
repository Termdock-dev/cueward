use std::thread;
use std::time::{Duration, Instant};

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::run_capture;
use super::script::{
    build_active_tab_script, build_close_script, build_exec_script_for_profile, build_open_script,
    build_tabs_script, decode_field, escape_js_string, extract_profile, parse_tab_line,
    parse_tabs_output, selector_click_js, selector_exists_js, selector_fill_js, selector_text_js,
};
use super::types::{
    SafariClickResult, SafariCloseResult, SafariEvalResult, SafariFillResult, SafariReadResult,
    SafariScrollReadChunk, SafariScrollReadResult, SafariScrollReadSnapshot, SafariScrollResult,
    SafariSourceResult, SafariTab, SafariWaitResult,
};

pub(super) fn execute_js(js_code: &str, context: &str) -> Result<String, MacosError> {
    execute_js_for_profile(js_code, None, context)
}

pub(super) fn execute_js_for_profile(
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

pub fn tabs(profile_filter: Option<&str>) -> Result<Vec<SafariTab>, MacosError> {
    with_safari_session(|| {
        let stdout = run_capture(&build_tabs_script(), "safari_tabs")?;
        let mut tabs = parse_tabs_output(&stdout);
        if let Some(profile) = profile_filter {
            tabs.retain(|tab| tab.profile.as_deref() == Some(profile));
        }
        Ok(tabs)
    })
}

pub fn active(profile_filter: Option<&str>) -> Result<Option<SafariTab>, MacosError> {
    with_safari_session(|| {
        let stdout = run_capture(&build_active_tab_script(profile_filter), "safari_active")?;
        Ok(parse_tab_line(stdout.trim()).map(|mut tab| {
            tab.profile = extract_profile(&tab.window_name, &tab.title);
            tab
        }))
    })
}

pub fn open(url: &str, profile_filter: Option<&str>) -> Result<Option<SafariTab>, MacosError> {
    with_safari_session(|| {
        let stdout = run_capture(&build_open_script(url, profile_filter), "safari_open")?;
        Ok(parse_tab_line(stdout.trim()).map(|mut tab| {
            tab.profile = extract_profile(&tab.window_name, &tab.title);
            tab
        }))
    })
}

/// Focus a specific tab by index or by matching URL/title substring.
/// This sets the matched tab as the current tab so subsequent operations target it.
pub fn focus_tab(
    tab_selector: &str,
    profile_filter: Option<&str>,
) -> Result<SafariTab, MacosError> {
    with_safari_session(|| {
        let all_tabs = tabs(profile_filter)?;
        if all_tabs.is_empty() {
            return Err(MacosError::Other("no Safari tabs found".to_string()));
        }

        let matched = if let Ok(index) = tab_selector.parse::<usize>() {
            all_tabs.into_iter().nth(index)
        } else {
            let query = tab_selector.to_lowercase();
            all_tabs.into_iter().find(|t| {
                t.url.to_lowercase().contains(&query) || t.title.to_lowercase().contains(&query)
            })
        };

        let tab = matched
            .ok_or_else(|| MacosError::Other(format!("no tab matching '{tab_selector}'")))?;

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
    })
}

pub fn close(index: Option<usize>) -> Result<SafariCloseResult, MacosError> {
    with_safari_session(|| {
        let stdout = run_capture(&build_close_script(index), "safari_close")?;
        Ok(SafariCloseResult {
            closed: stdout.trim() == "true",
            index,
        })
    })
}

pub fn close_tabs(
    profile_filter: Option<&str>,
    url_pattern: Option<&str>,
) -> Result<usize, MacosError> {
    with_safari_session(|| {
        let all_tabs = tabs(profile_filter)?;
        let to_close: Vec<&SafariTab> = all_tabs
            .iter()
            .filter(|tab| match url_pattern {
                Some(pattern) => tab.url.contains(pattern),
                None => true,
            })
            .collect();

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
    })
}

pub fn source(profile_filter: Option<&str>) -> Result<SafariSourceResult, MacosError> {
    with_safari_session(|| {
        let result = execute_js_for_profile(
            "document.documentElement.outerHTML",
            profile_filter,
            "safari_source",
        )?;
        Ok(SafariSourceResult { html: result })
    })
}

pub fn read(
    selector: Option<&str>,
    profile_filter: Option<&str>,
) -> Result<SafariReadResult, MacosError> {
    with_safari_session(|| {
        let js = match selector {
            Some(selector) => selector_text_js(selector),
            None => "(document.body.innerText ?? \"\").trim()".to_string(),
        };
        let content = execute_js_for_profile(&js, profile_filter, "safari_read")?;
        Ok(SafariReadResult {
            selector: selector.map(ToOwned::to_owned),
            content,
        })
    })
}

pub fn exec(js_code: &str, profile_filter: Option<&str>) -> Result<SafariEvalResult, MacosError> {
    with_safari_session(|| {
        let result = execute_js_for_profile(js_code, profile_filter, "safari_exec")?;
        Ok(SafariEvalResult { result })
    })
}

pub fn click(selector: &str) -> Result<SafariClickResult, MacosError> {
    with_safari_session(|| {
        let result = execute_js(&selector_click_js(selector), "safari_click")?;
        if result.trim() != "true" {
            return Err(MacosError::Other(result));
        }
        Ok(SafariClickResult {
            clicked: true,
            selector: selector.to_string(),
        })
    })
}

pub fn fill(selector: &str, text: &str) -> Result<SafariFillResult, MacosError> {
    with_safari_session(|| {
        let result = execute_js(&selector_fill_js(selector, text), "safari_fill")?;
        if result.trim() != "true" {
            return Err(MacosError::Other(result));
        }
        Ok(SafariFillResult {
            filled: true,
            selector: selector.to_string(),
            text: text.to_string(),
        })
    })
}

pub fn wait(selector: &str, timeout_seconds: u64) -> Result<SafariWaitResult, MacosError> {
    with_safari_session(|| {
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
    })
}

fn block_fingerprint(text: &str) -> String {
    text.chars().take(120).collect()
}

pub(super) fn scroll_read_new_content_blocks(content: &str, seen: &[String]) -> Vec<String> {
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

pub(super) fn scroll_read_snapshot_blocks(
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

pub(super) fn scroll_read_detects_new_content(
    previous_text: &str,
    previous_count: usize,
    current_text: &str,
    current_count: usize,
) -> bool {
    current_count > previous_count || current_text.trim() != previous_text.trim()
}

pub fn scroll(
    direction: &str,
    amount: Option<i64>,
    profile_filter: Option<&str>,
) -> Result<SafariScrollResult, MacosError> {
    with_safari_session(|| {
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
    })
}

pub(super) fn scroll_read_poll_js(selector: Option<&str>) -> String {
    let scope_expr = match selector {
        Some(selector) => format!("document.querySelector(\"{}\")", escape_js_string(selector)),
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
            let itemNodes = [
              ...scope.querySelectorAll("shreddit-post"),
              ...scope.querySelectorAll("[data-testid='post-container']"),
              ...scope.querySelectorAll("article"),
              ...scope.querySelectorAll("[role='article']")
            ];
            if (itemNodes.length === 0) {{
              const MIN_TEXT = 50;
              const MAX_TEXT = 3000;
              itemNodes = [...scope.querySelectorAll("div")].filter(el => {{
                const t = (el.innerText || "").trim();
                if (t.length < MIN_TEXT || t.length > MAX_TEXT) return false;
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
    serde_json::from_str(payload).map_err(|error| {
        MacosError::Other(format!("invalid scroll/read payload: {error}: {payload}"))
    })
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
    with_safari_session(|| {
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
    })
}
