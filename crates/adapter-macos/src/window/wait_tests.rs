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
