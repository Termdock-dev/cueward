use super::TAB_SEPARATOR;
use super::script::{
    build_active_tab_script, build_close_script, build_exec_script, build_open_script,
    build_tab_return_block, build_tabs_script, parse_tab_line, parse_tabs_output,
    selector_click_js, selector_fill_js, selector_text_js,
};

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
