use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::interaction::RUNTIME;
use super::target::{execute_js_in_tab, resolve_tab};

static NEXT_SNAPSHOT_ID: AtomicU64 = AtomicU64::new(1);

fn inspect_js(limit: usize, snapshot: &str) -> String {
    let snapshot = serde_json::json!(snapshot);
    format!(
        r#"(() => {{
          {RUNTIME}
          const state = {{snapshot: {snapshot}, nodes: new Map()}};
          window.__cuewardRefState = state;
          const nodes = [];
          let skippedFrames = 0;
          cuewardVisitRoots(document, root => {{
            for (const el of root.querySelectorAll?.('*') || []) {{
              if (el.tagName === 'IFRAME') {{
                try {{ if (!el.contentDocument) skippedFrames += 1; }}
                catch (_) {{ skippedFrames += 1; }}
              }}
              if (nodes.length >= {limit} || el.getClientRects?.().length === 0) continue;
              const tag = (el.tagName || '').toLowerCase();
              const role = el.getAttribute?.('role') || null;
              const interactive = ['a', 'button', 'input', 'textarea', 'select'].includes(tag)
                || ['button', 'link', 'textbox', 'checkbox', 'radio', 'slider', 'menuitem'].includes(role);
              const hasTextChild = [...el.children || []].some(child =>
                (child.innerText || '').trim().length > 0);
              const text = (el.innerText || el.textContent || '').trim().slice(0, 160);
              if (!interactive && (!text || hasTextChild)) continue;
              const labelledBy = (el.getAttribute?.('aria-labelledby') || '').split(/\s+/)
                .map(id => el.ownerDocument.getElementById(id)?.innerText || '').join(' ').trim();
              const labels = Array.from(el.labels || [])
                .map(label => label.innerText || '').join(' ').trim();
              const ref = `${{state.snapshot}}:${{state.nodes.size + 1}}`;
              state.nodes.set(ref, el);
              nodes.push({{
                ref, tag, role,
                name: (el.getAttribute?.('aria-label') || labelledBy || labels ||
                  el.getAttribute?.('title') || el.getAttribute?.('placeholder') || text).slice(0, 160),
                text, href: el.getAttribute?.('href') || null
              }});
            }}
          }});
          return JSON.stringify({{
            url: location.href, title: document.title, snapshot: state.snapshot,
            nodes, skipped_frames: skippedFrames
          }});
        }})()"#
    )
}

/// Read an agent-friendly outline and assign short-lived element references.
pub fn inspect(
    limit: usize,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<Value, MacosError> {
    if limit == 0 || limit > 1000 {
        return Err(MacosError::Other(
            "inspect limit must be 1..=1000".to_string(),
        ));
    }
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| MacosError::Other(format!("system clock before epoch: {error}")))?;
        let snapshot = format!(
            "{:x}-{:x}-{:x}",
            clock.as_nanos(),
            std::process::id(),
            NEXT_SNAPSHOT_ID.fetch_add(1, Ordering::Relaxed)
        );
        let raw = execute_js_in_tab(&inspect_js(limit, &snapshot), &tab, "safari_inspect")?;
        serde_json::from_str(&raw)
            .map_err(|error| MacosError::Other(format!("invalid inspect response: {error}")))
    })
}

fn batch_js(steps: &Value) -> Result<String, MacosError> {
    let steps = serde_json::to_string(steps)
        .map_err(|error| MacosError::Other(format!("invalid batch steps: {error}")))?;
    Ok(format!(
        r#"(() => {{
          {RUNTIME}
          const steps = {steps};
          const completed = [];
          for (let index = 0; index < steps.length; index++) {{
            const step = steps[index];
            try {{
              if (step.action === 'assert') {{
                if (step.js != null && typeof step.js !== 'string')
                  throw new Error('assert js must be a string');
                const observed = step.js != null
                  ? (0, eval)(step.js) : cuewardResolve(step);
                if (observed?.then) throw new Error('assert requires a synchronous condition');
                const passes = Boolean(observed);
                if (!passes) throw new Error('assertion failed');
              }} else {{
                const el = cuewardResolve(step);
                switch (step.action) {{
                  case 'click': cuewardClick(el); break;
                  case 'fill':
                    if (typeof step.value !== 'string') throw new Error('fill value must be a string');
                    cuewardFill(el, step.value); break;
                  case 'key':
                    if (typeof step.key !== 'string' || !step.key) throw new Error('key must be a string');
                    cuewardKey(el, step.key, step); break;
                  case 'select':
                    if (typeof step.value !== 'string') throw new Error('select value must be a string');
                    cuewardSelect(el, step.value); break;
                  case 'check':
                    if (typeof step.checked !== 'boolean') throw new Error('checked must be a boolean');
                    cuewardCheck(el, step.checked); break;
                  case 'scroll_into_view': cuewardScrollIntoView(el); break;
                  default: throw new Error('unknown action: ' + step.action);
                }}
              }}
              completed.push({{index, action: step.action}});
            }} catch (error) {{
              return JSON.stringify({{ok: false, failed_step: index,
                error: String(error?.message || error), completed}});
            }}
          }}
          return JSON.stringify({{ok: true, completed}});
        }})()"#
    ))
}

/// Execute a sequence of page actions in one JavaScript call.
pub fn batch(
    steps: &Value,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<Value, MacosError> {
    let steps_array = steps
        .as_array()
        .ok_or_else(|| MacosError::Other("batch steps must be a JSON array".to_string()))?;
    if steps_array.is_empty() || steps_array.len() > 100 {
        return Err(MacosError::Other(
            "batch requires 1..=100 steps".to_string(),
        ));
    }
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        let raw = execute_js_in_tab(&batch_js(steps)?, &tab, "safari_batch")?;
        serde_json::from_str(&raw)
            .map_err(|error| MacosError::Other(format!("invalid batch response: {error}")))
    })
}
