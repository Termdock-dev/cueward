use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::run_capture;
use super::interaction::{selector_click_js, selector_fill_js};
use super::script::{
    build_active_tab_script, build_close_script, build_exec_script_for_profile, build_open_script,
    build_tabs_script, decode_field, extract_profile, parse_tab_line,
    parse_tabs_output, selector_text_js,
};
use super::target::{execute_js_in_tab, resolve_tab};
use super::types::{
    SafariClickResult, SafariCloseResult, SafariFillResult, SafariReadResult, SafariSourceResult,
    SafariTab,
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

pub(crate) fn doctor_live_probe() -> Result<String, MacosError> {
    with_safari_session(|| execute_js("1 + 1", "safari_doctor_live"))
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

pub fn source(
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<SafariSourceResult, MacosError> {
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        let result = execute_js_in_tab("document.documentElement.outerHTML", &tab, "safari_source")?;
        Ok(SafariSourceResult { html: result })
    })
}

pub fn read(
    selector: Option<&str>,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<SafariReadResult, MacosError> {
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        let js = match selector {
            Some(selector) => selector_text_js(selector),
            None => "(document.body.innerText ?? \"\").trim()".to_string(),
        };
        let content = execute_js_in_tab(&js, &tab, "safari_read")?;
        Ok(SafariReadResult {
            selector: selector.map(ToOwned::to_owned),
            content,
        })
    })
}

pub fn click(
    selector: &str,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<SafariClickResult, MacosError> {
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        let result = execute_js_in_tab(&selector_click_js(selector), &tab, "safari_click")?;
        if result.trim() != "true" {
            return Err(MacosError::Other(result));
        }
        Ok(SafariClickResult {
            clicked: true,
            selector: selector.to_string(),
        })
    })
}

pub fn fill(
    selector: &str,
    text: &str,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<SafariFillResult, MacosError> {
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        let result = execute_js_in_tab(&selector_fill_js(selector, text), &tab, "safari_fill")?;
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
