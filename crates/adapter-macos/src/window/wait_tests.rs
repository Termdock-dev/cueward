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
fn wait_reports_missing_role_for_named_or_unqualified_element_queries() {
    let mut request = options(WaitCondition::ElementExists);
    for selector in [
        None,
        Some(WaitSelector {
            role: String::new(),
            name: Some("Continue".into()),
            identifier: None,
        }),
        Some(WaitSelector {
            role: String::new(),
            name: None,
            identifier: Some("status".into()),
        }),
    ] {
        request.selector = selector;
        assert_eq!(
            validate(&request).expect_err("role required").to_string(),
            "element conditions require --role"
        );
    }
}

#[test]
fn wait_classifies_window_lifecycle_from_consistent_catalog_observations() {
    let directory = tempfile::tempdir().expect("wait catalog fixture");
    let path = directory.path().join("wait.swift");
    fs::write(
        &path,
        [
            include_str!("ax_elements.swift"),
            include_str!("ax_shared.swift"),
            include_str!("ax_common.swift"),
            include_str!("wait_catalog_fixture.swift"),
            include_str!("wait_logic.swift"),
            include_str!("ax_wait.swift"),
        ]
        .join("\n"),
    )
    .expect("write catalog fixture");
    let request = serde_json::to_vec(&json!({
        "caller_pid": std::process::id(),
        "options": { "condition": "window-gone", "timeout_ms": 200, "interval_ms": 50 }
    }))
    .expect("request");
    for (scenario, expected) in [
        ("close", "matched"),
        ("absent", "matched"),
        ("present", "timed_out"),
        ("rename", "window_changed"),
        ("owner", "window_changed"),
        ("bounds", "window_changed"),
        ("fractional", "timed_out"),
        ("unavailable", "error"),
    ] {
        let output = crate::window::process::run_with_timeout(
            Command::new("swift")
                .arg(&path)
                .args(["123", "7", "Fixture", "-100", "200", "640", "480", scenario]),
            &request,
            std::time::Duration::from_secs(40),
        )
        .expect("run production wait loop");
        let stderr = String::from_utf8_lossy(&output.stderr);
        if expected == "error" {
            assert!(!output.status.success());
            assert!(stderr.contains("absence is unproven"), "{stderr}");
        } else {
            assert!(output.status.success(), "{scenario}: {stderr}");
            let result: serde_json::Value =
                serde_json::from_slice(&output.stdout).expect("wait result");
            assert_eq!(result["status"], expected, "{scenario}");
        }
    }
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
