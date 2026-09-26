use super::*;
use std::fs;
use std::process::Command;

fn options(condition: WaitCondition) -> WaitOptions {
    WaitOptions {
        condition,
        selector: Some(WaitSelector {
            role: "AXTextField".into(),
            name: Some("Status".into()),
            identifier: None,
        }),
        value: None,
        timeout_ms: 4000,
        interval_ms: 100,
    }
}

#[test]
fn wait_options_reject_incomplete_and_unbounded_queries() {
    let mut request = options(WaitCondition::Enabled);
    assert!(validate(&request).is_ok());
    request.timeout_ms = 20_001;
    assert!(validate(&request).is_err());
    request.timeout_ms = 100;
    request.interval_ms = 0;
    assert!(validate(&request).is_err());
    request.interval_ms = 100;
    request.condition = WaitCondition::ValueEquals;
    assert!(validate(&request).is_err());
    request.value = Some(String::new());
    assert!(validate(&request).is_ok());
    request.condition = WaitCondition::WindowGone;
    assert!(validate(&request).is_err());
    request.selector = None;
    request.value = None;
    assert!(validate(&request).is_ok());
    request.condition = WaitCondition::ElementAbsent;
    assert!(validate(&request).is_err());
}

#[test]
fn wait_matching_requires_exact_values_unique_nodes_and_complete_absence() {
    let directory = tempfile::tempdir().expect("wait fixture");
    let path = directory.path().join("wait.swift");
    fs::write(
        &path,
        format!(
            "{}\n{}",
            include_str!("wait_logic.swift"),
            include_str!("wait_logic_tests.swift")
        ),
    )
    .expect("write wait fixture");
    let output = Command::new("swift")
        .arg(path)
        .output()
        .expect("run wait matching");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "passed");
}

fn missing_window_target() -> String {
    use crate::screenshot::{CapturableWindow, WindowBounds};
    use crate::window::snapshot::SnapshotImage;
    let window = CapturableWindow {
        window_id: 999_999_999,
        owner_pid: std::process::id() as i32,
        app: "Wait Validation Fixture".into(),
        title: "Missing fixture window".into(),
        is_frontmost: false,
        is_onscreen: false,
        bounds: WindowBounds {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
    };
    let image = SnapshotImage {
        width: 100,
        height: 100,
        scale_x: 1.0,
        scale_y: 1.0,
        origin: "top-left",
    };
    InputTarget::issue(&window, &image).expect("fixture token")
}

#[test]
#[ignore = "requires Accessibility and Screen Recording permissions; sends no input"]
fn wait_rejects_unmatchable_names_before_observing_window() {
    let target = missing_window_target();
    for grapheme in ["a", "測", "e\u{301}"] {
        let mut request = options(WaitCondition::ElementAbsent);
        request.selector.as_mut().expect("selector").name = Some(grapheme.repeat(513));
        let error = wait_for_window(&target, &request).expect_err("unmatchable name rejected");
        assert!(error.to_string().contains("512"), "{error}");
    }
}

#[test]
#[ignore = "requires Accessibility and Screen Recording permissions; sends no input"]
fn wait_name_limit_counts_graphemes_and_preserves_full_identifiers() {
    let target = missing_window_target();
    let mut request = options(WaitCondition::ElementAbsent);
    let selector = request.selector.as_mut().expect("selector");
    selector.name = Some("e\u{301}".repeat(512));
    selector.identifier = Some("i".repeat(600));
    assert_eq!(
        wait_for_window(&target, &request)
            .expect("valid selector reaches catalog")
            .status,
        WaitStatus::WindowGone
    );
}
