use crate::MacosError;
use crate::applescript::escape_body;

use super::core::{active, tabs};
use super::map_js_timeout;
use super::run_capture;
use super::script::{decode_field, js_apple_event_command, safari_script_prelude};
use super::types::SafariTab;

pub(super) fn resolve_tab(
    selector: Option<&str>,
    profile_filter: Option<&str>,
) -> Result<SafariTab, MacosError> {
    let Some(selector) = selector else {
        return active(profile_filter)?
            .ok_or_else(|| MacosError::Other("no Safari tab available".to_string()));
    };
    let all_tabs = tabs(profile_filter)?;
    let matched = if let Ok(index) = selector.parse::<usize>() {
        all_tabs.into_iter().nth(index)
    } else {
        let query = selector.to_lowercase();
        all_tabs.into_iter().find(|tab| {
            tab.url.to_lowercase().contains(&query) || tab.title.to_lowercase().contains(&query)
        })
    };
    matched.ok_or_else(|| MacosError::Other(format!("no tab matching '{selector}'")))
}

pub(super) fn execute_js_in_tab(
    js_code: &str,
    tab: &SafariTab,
    context: &str,
) -> Result<String, MacosError> {
    let js_expr = escape_body(js_code);
    let js_command = js_apple_event_command(&format!("tab {} of w", tab.index + 1), None);
    let script = format!(
        r#"
        {prelude}
        tell application "Safari"
          repeat with w in every window
            if (id of w) is {window_id} then
              if (count of tabs of w) < {tab_index} then error "target tab closed"
              set jsCode to {js_expr}
              {js_command}
              if rawResult is missing value then error "JavaScript did not return a string"
              return my encode_field(rawResult as string)
            end if
          end repeat
          error "target window closed"
        end tell
        "#,
        prelude = safari_script_prelude(),
        window_id = tab.window_id,
        tab_index = tab.index + 1,
        js_command = js_command,
    );
    let output = run_capture(&script, context).map_err(|error| {
        map_js_timeout(
            error,
            &format!("window {} tab index {}", tab.window_id, tab.index),
        )
    })?;
    Ok(decode_field(output.trim()))
}
