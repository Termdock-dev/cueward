use crate::applescript::{escape, escape_body};

use super::types::SafariTab;
use super::{FIELD_SEPARATOR, TAB_SEPARATOR};

pub(super) fn decode_field(value: &str) -> String {
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

pub(super) fn escape_js_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

pub(super) fn extract_profile(window_name: &str, active_tab_title: &str) -> Option<String> {
    let expected_suffix = format!(" — {active_tab_title}");
    window_name
        .strip_suffix(&expected_suffix)
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn target_window_block(profile_filter: Option<&str>) -> String {
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

pub(super) fn parse_tab_line(line: &str) -> Option<SafariTab> {
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

pub(super) fn parse_tabs_output(stdout: &str) -> Vec<SafariTab> {
    let mut tabs: Vec<SafariTab> = stdout
        .split(TAB_SEPARATOR)
        .filter(|line| !line.trim().is_empty())
        .filter_map(parse_tab_line)
        .collect();

    let mut profiles_by_window = std::collections::HashMap::new();
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

pub(super) fn safari_script_prelude() -> String {
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

pub(super) fn build_tab_return_block(tab_ref: &str, active_flag: &str) -> String {
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

pub(super) fn build_tabs_script() -> String {
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

pub(super) fn build_active_tab_script(profile_filter: Option<&str>) -> String {
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

pub(super) fn build_exec_script_for_profile(js_code: &str, profile_filter: Option<&str>) -> String {
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

pub(super) fn build_open_script(url: &str, profile_filter: Option<&str>) -> String {
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

pub(super) fn build_close_script(index: Option<usize>) -> String {
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
pub(super) fn build_exec_script(js_code: &str) -> String {
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

pub(super) fn selector_text_js(selector: &str) -> String {
    let selector = escape_js_string(selector);
    format!(
        r#"(() => {{
            const el = document.querySelector("{selector}");
            if (!el) throw new Error("selector not found");
            return (el.innerText ?? el.textContent ?? "").trim();
        }})()"#
    )
}

pub(super) fn selector_exists_js(selector: &str) -> String {
    let selector = escape_js_string(selector);
    format!(r#"(() => document.querySelector("{selector}") ? "true" : "false")()"#)
}

pub(super) fn selector_click_js(selector: &str) -> String {
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

pub(super) fn selector_fill_js(selector: &str, text: &str) -> String {
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
