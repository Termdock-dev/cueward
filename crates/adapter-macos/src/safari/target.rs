use crate::MacosError;
use crate::applescript::{escape, escape_body};

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
    select_tab(tabs(profile_filter)?, selector)
}

pub(super) fn select_tab(
    all_tabs: Vec<SafariTab>,
    selector: &str,
) -> Result<SafariTab, MacosError> {
    if let Ok(index) = selector.parse::<usize>() {
        return all_tabs
            .into_iter()
            .nth(index)
            .ok_or_else(|| MacosError::Other(format!("no tab matching '{selector}'")));
    }

    let query = selector.to_lowercase();
    let mut matching = all_tabs.into_iter().filter(|tab| {
        tab.url.to_lowercase().contains(&query) || tab.title.to_lowercase().contains(&query)
    });
    let selected = matching
        .next()
        .ok_or_else(|| MacosError::Other(format!("no tab matching '{selector}'")))?;
    if matching.next().is_some() {
        return Err(MacosError::Other(format!(
            "tab selector is ambiguous: '{selector}'; use a unique URL/title or numeric --tab index"
        )));
    }
    Ok(selected)
}

pub(super) fn tab_identity_guard(tab: &SafariTab) -> String {
    format!(
        r#"if (count of tabs of w) < {tab_index} then error "target tab closed"
              set targetTab to tab {tab_index} of w
              if (URL of targetTab) is not "{url}" then error "target tab changed; list tabs and retry"
              if (name of targetTab) is not "{title}" then error "target tab changed; list tabs and retry""#,
        tab_index = tab.index + 1,
        url = escape(&tab.url),
        title = escape(&tab.title),
    )
}

pub(super) fn execute_js_in_tab(
    js_code: &str,
    tab: &SafariTab,
    context: &str,
) -> Result<String, MacosError> {
    let js_expr = escape_body(js_code);
    let js_command = js_apple_event_command("targetTab", None);
    let identity_guard = tab_identity_guard(tab);
    let script = format!(
        r#"
        {prelude}
        tell application "Safari"
          repeat with w in every window
            if (id of w) is {window_id} then
              {identity_guard}
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
        identity_guard = identity_guard,
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

#[cfg(test)]
mod tests {
    use super::{SafariTab, select_tab};

    fn tab(index: usize, title: &str, url: &str) -> SafariTab {
        SafariTab {
            window_id: 42,
            window_name: "Work".to_string(),
            profile: Some("Work".to_string()),
            index,
            title: title.to_string(),
            url: url.to_string(),
            active: index == 0,
        }
    }

    #[test]
    fn text_selector_rejects_multiple_matching_tabs() {
        let tabs = vec![
            tab(0, "Docs", "https://example.test/one"),
            tab(1, "Docs", "https://example.test/two"),
        ];
        let error = select_tab(tabs, "Docs").expect_err("ambiguous title must fail");
        assert!(error.to_string().contains("ambiguous"));
    }

    #[test]
    fn text_selector_returns_its_only_match() {
        let tabs = vec![
            tab(0, "Docs", "https://example.test/one"),
            tab(1, "Inbox", "https://example.test/two"),
        ];
        let selected = select_tab(tabs, "Inbox").expect("unique title");
        assert_eq!(selected.index, 1);
    }

    #[test]
    fn numeric_selector_chooses_explicit_list_index() {
        let tabs = vec![
            tab(0, "Docs", "https://example.test/one"),
            tab(1, "Docs", "https://example.test/two"),
        ];
        let selected = select_tab(tabs, "1").expect("explicit index");
        assert_eq!(selected.url, "https://example.test/two");
    }

    #[test]
    #[ignore = "requires an open Safari tab and JavaScript from Apple Events; read-only probe"]
    fn identity_guard_rejects_a_case_only_url_change() {
        let mut selected = super::active(None).expect("read active tab").expect("open tab");
        let changed = selected.url.to_uppercase();
        assert_ne!(selected.url, changed, "fixture URL must contain lowercase letters");
        selected.url = changed;
        let error = crate::safari_guard::with_safari_session(|| {
            super::execute_js_in_tab("'cueward-read-only-probe'", &selected, "identity_guard_probe")
        }).expect_err("a changed URL must be rejected before JavaScript executes");
        assert!(error.to_string().contains("target tab changed"), "{error}");
    }
}
