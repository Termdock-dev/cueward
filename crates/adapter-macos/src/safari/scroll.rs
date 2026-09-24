use std::thread;
use std::time::{Duration, Instant};

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::script::escape_js_string;
use super::target::{execute_js_in_tab, resolve_tab};
use super::types::{
    SafariScrollReadChunk, SafariScrollReadResult, SafariScrollReadSnapshot, SafariScrollResult,
    SafariTab,
};

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
    tab_selector: Option<&str>,
) -> Result<SafariScrollResult, MacosError> {
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        scroll_in_tab(direction, amount, &tab)
    })
}

fn scroll_in_tab(
    direction: &str,
    amount: Option<i64>,
    tab: &SafariTab,
) -> Result<SafariScrollResult, MacosError> {
    let pixels = amount.unwrap_or(500).unsigned_abs();
    let js = match direction {
            "down" => format!("(function(){{ window.scrollBy(0, {pixels}); return JSON.stringify({{ x: Math.round(window.scrollX), y: Math.round(window.scrollY) }}); }})()"),
            "up" => format!("(function(){{ window.scrollBy(0, -{pixels}); return JSON.stringify({{ x: Math.round(window.scrollX), y: Math.round(window.scrollY) }}); }})()"),
            "top" => "(function(){ window.scrollTo(0, 0); return JSON.stringify({ x: 0, y: 0 }); })()".to_string(),
            "bottom" => "(function(){ window.scrollTo(0, document.body.scrollHeight); return JSON.stringify({ x: Math.round(window.scrollX), y: Math.round(window.scrollY) }); })()".to_string(),
            _ => return Err(MacosError::Other(format!("unknown scroll direction: {direction}. Use: up, down, top, bottom"))),
        };
    let raw = execute_js_in_tab(&js, tab, "safari_scroll")?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| MacosError::Other(format!("failed to parse scroll result: {e}")))?;
    Ok(SafariScrollResult {
        scroll_x: value.get("x").and_then(|v| v.as_i64()).unwrap_or(0),
        scroll_y: value.get("y").and_then(|v| v.as_i64()).unwrap_or(0),
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
    tab: &SafariTab,
    timeout: Duration,
) -> Result<SafariScrollReadSnapshot, MacosError> {
    let deadline = Instant::now() + timeout;
    let js = scroll_read_poll_js(selector);

    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(500));
        let payload = execute_js_in_tab(&js, tab, "safari_scroll_read_poll")?;
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

    let payload = execute_js_in_tab(&js, tab, "safari_scroll_read_poll")?;
    parse_scroll_read_snapshot(&payload)
}

pub fn scroll_and_read(
    times: u64,
    amount: Option<i64>,
    selector: Option<&str>,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<SafariScrollReadResult, MacosError> {
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        let js = scroll_read_poll_js(selector);
        let payload = execute_js_in_tab(&js, &tab, "safari_scroll_read_initial")?;
        let mut snapshot = parse_scroll_read_snapshot(&payload)?;
        let mut seen = scroll_read_snapshot_blocks(&snapshot, &[]);
        let mut chunks = Vec::new();

        for iteration in 1..=times {
            scroll_in_tab("down", amount, &tab)?;
            snapshot =
                poll_scroll_read_snapshot(selector, &snapshot, &tab, Duration::from_secs(5))?;
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
