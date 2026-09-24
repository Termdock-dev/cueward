use super::TAB_SEPARATOR;
use super::interaction::{RUNTIME, selector_click_js, selector_fill_js};
use super::script::{
    build_active_tab_script, build_close_script, build_exec_script,
    build_exec_script_for_profile, build_open_script, build_tab_return_block, build_tabs_script,
    js_apple_event_command, parse_tab_line, parse_tabs_output, selector_text_js,
};
use std::process::Command;

#[test]
fn node_runtime_is_available_for_browser_behavior_tests() {
    let output = Command::new("node")
        .arg("--version")
        .output()
        .expect("Node.js is required for Safari JavaScript behavior tests");
    assert!(output.status.success(), "Node.js --version failed");
}

fn run_browser_builder(setup: &str, action: &str, result: &str) -> String {
    let script = format!(
        "class PointerEvent extends Event {{ constructor(type, options) {{ super(type, options); }} }} \
         class MouseEvent extends Event {{ constructor(type, options) {{ super(type, options); }} }} \
         {setup}; {action}; process.stdout.write(String({result}));"
    );
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .output()
        .expect("Node.js is required for Safari JavaScript behavior tests");
    assert!(
        output.status.success(),
        "browser action failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 result")
}

#[test]
fn click_reaches_pointerdown_handlers() {
    let setup = r#"
      const button = new EventTarget();
      let opened = false;
      button.click = () => button.dispatchEvent(new MouseEvent('click'));
      button.addEventListener('pointerdown', () => { opened = true; });
      globalThis.document = { querySelector: () => button };
    "#;
    let result = run_browser_builder(setup, &selector_click_js("#trigger"), "opened");
    assert_eq!(result, "true");
}

#[test]
fn fill_notifies_framework_value_tracker() {
    let setup = r#"
      class Input extends EventTarget {
        constructor() {
          super(); this.storedValue = ''; this.trackedValue = '';
          Object.defineProperty(this, 'value', {
            get() { return this.storedValue; },
            set(value) { this.storedValue = value; this.trackedValue = value; }
          });
        }
      }
      Object.defineProperty(Input.prototype, 'value', {
        get() { return this.storedValue; },
        set(value) { this.storedValue = value; }
      });
      globalThis.HTMLInputElement = Input;
      globalThis.HTMLTextAreaElement = class extends Input {};
      const input = new Input();
      let accepted = '';
      input.addEventListener('input', () => {
        if (input.value !== input.trackedValue) accepted = input.value;
      });
      globalThis.document = { querySelector: () => input };
    "#;
    let result = run_browser_builder(setup, &selector_fill_js("#field", "new value"), "accepted");
    assert_eq!(result, "new value");
}

#[test]
fn iframe_form_controls_use_their_own_dom_realm() {
    let script = format!(
        r#"
        globalThis.window = globalThis;
        globalThis.HTMLInputElement = class {{}};
        globalThis.HTMLTextAreaElement = class {{}};
        globalThis.HTMLSelectElement = class {{}};
        class FrameInput extends EventTarget {{ constructor(type = 'text') {{ super(); this.type = type; this.stored = ''; this.marked = false; }} }}
        Object.defineProperty(FrameInput.prototype, 'value', {{get() {{ return this.stored; }}, set(v) {{ this.stored = v; }}}});
        Object.defineProperty(FrameInput.prototype, 'checked', {{get() {{ return this.marked; }}, set(v) {{ this.marked = v; }}}});
        class FrameTextarea extends FrameInput {{}}
        class FrameSelect extends EventTarget {{ constructor() {{ super(); this.value = 'a'; }} }}
        const frame = {{HTMLInputElement: FrameInput, HTMLTextAreaElement: FrameTextarea,
          HTMLSelectElement: FrameSelect, Event}};
        const selection = {{removeAllRanges() {{}}, addRange() {{}}}};
        const frameDoc = {{defaultView: frame, getSelection: () => selection,
          createRange: () => ({{selectNodeContents() {{}}}}),
          execCommand: () => true}};
        const input = new FrameInput(); input.ownerDocument = frameDoc;
        const select = new FrameSelect(); select.ownerDocument = frameDoc;
        const check = new FrameInput('checkbox'); check.ownerDocument = frameDoc;
        const editable = {{isContentEditable: true, ownerDocument: frameDoc, focus() {{}}}};
        {RUNTIME}
        const run = fn => {{ try {{ fn(); return true; }} catch (_) {{ return false; }} }};
        process.stdout.write(JSON.stringify({{
          fill: run(() => cuewardFill(input, 'new')) && input.value === 'new',
          select: run(() => cuewardSelect(select, 'b')) && select.value === 'b',
          check: run(() => cuewardCheck(check, true)) && check.checked === true,
          editable: run(() => cuewardFill(editable, 'text'))
        }}));
        "#
    );
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .output()
        .expect("Node.js is required for Safari JavaScript behavior tests");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("result");
    assert_eq!(
        result,
        serde_json::json!({"fill":true,"select":true,"check":true,"editable":true})
    );
}

#[test]
fn parse_tab_line_decodes_fields() {
    let line = "61998<<<FIELD_SEP>>>Work — Google\\tGemini<<<FIELD_SEP>>>0<<<FIELD_SEP>>>Google\\tGemini<<<FIELD_SEP>>>https://gemini.google.com/app<<<FIELD_SEP>>>true";

    let tab = parse_tab_line(line).expect("tab");

    assert_eq!(tab.window_id, 61998);
    assert_eq!(tab.window_name, "Work — Google\tGemini");
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
fn javascript_apple_event_has_a_scoped_timeout() {
    let script = js_apple_event_command("tab 2 of w");

    assert!(script.contains("with timeout of 15 seconds"));
    assert!(script.contains("do JavaScript jsCode in tab 2 of w"));
    assert!(script.contains("CUEWARD_JS_APPLE_EVENT_TIMEOUT"));
    assert!(script.contains("if errNum is -1712"));
}

#[test]
fn profile_javascript_timeout_carries_the_selected_tab_identity() {
    let script = build_exec_script_for_profile("1 + 1", Some("Work"));

    assert!(script.contains("set targetTab to current tab of w"));
    assert!(script.contains("set targetWindowId to (id of w) as text"));
    assert!(script.contains("set targetTabIndex to ((index of targetTab) - 1) as text"));
    assert!(script.contains("do JavaScript jsCode in targetTab"));
    assert!(script.contains("targetWindowId & \"|\" & targetTabIndex"));
}

#[test]
fn selector_js_builders_include_selector_and_text() {
    assert!(selector_text_js(".item").contains("querySelector(\".item\")"));
}
