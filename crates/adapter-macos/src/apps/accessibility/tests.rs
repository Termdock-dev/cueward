use super::*;
use crate::window::process::run_with_timeout;
use std::io::Write;
use std::process::Command;
use std::time::Duration;

#[test]
fn app_action_helper_typechecks_without_desktop_permissions() {
    let mut script = tempfile::NamedTempFile::with_suffix(".swift").unwrap();
    write!(script, "{}", source(include_str!("app_action.swift"))).unwrap();
    let output = Command::new("swiftc")
        .arg("-typecheck")
        .arg(script.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rejects_invalid_requests_before_starting_an_ax_helper() {
    for (pid, root, limit, depth) in [
        (0, None, 100, 3),
        (1, None, 0, 3),
        (1, None, 501, 3),
        (1, None, 100, 0),
        (1, None, 100, 13),
        (1, Some("window"), 100, 3),
    ] {
        assert!(inspect_app(pid, root, limit, depth).is_err());
    }
    assert!(press_app_element("invalid").is_err());
    let error = set_app_value("invalid", &"a".repeat(65_537)).unwrap_err();
    assert!(error.to_string().contains("65536"));
}

fn inspect_fixture(scenario: &str, root: Option<&str>) -> Result<serde_json::Value, String> {
    let mut script = tempfile::NamedTempFile::with_suffix(".swift").unwrap();
    write!(
        script,
        "{}\n{}",
        source(include_str!("root_fixture.swift")),
        include_str!("app_inspect.swift")
    )
    .unwrap();
    let input = serde_json::to_vec(&json!({"pid": std::process::id(), "root": root,
        "limit": 100, "depth": 3}))
    .unwrap();
    let output = run_with_timeout(
        Command::new("swift")
            .arg(script.path())
            .env("APP_AX_SCENARIO", scenario),
        &input,
        Duration::from_secs(40),
    )
    .unwrap();
    if output.status.success() {
        Ok(serde_json::from_slice(&output.stdout).unwrap())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

#[test]
fn app_roots_support_windowless_menus_fallbacks_and_service_descendants() {
    let menu = inspect_fixture("windowless", Some("menu")).unwrap();
    assert_eq!(menu["nodes"][1]["role"], "AXMenuItem");
    for scenario in ["fallback", "deduplicate", "service"] {
        let snapshot = inspect_fixture(scenario, Some("w0")).unwrap();
        assert_eq!(snapshot["nodes"][0]["ref"], "w0");
        assert_eq!(snapshot["nodes"][1]["ref"], "w0.0");
        assert_eq!(snapshot["nodes"][1]["value"], "Fixture text");
        if scenario == "service" {
            assert_ne!(
                snapshot["nodes"][0]["receiver_pid"],
                snapshot["nodes"][1]["receiver_pid"]
            );
            continue;
        }
        let subtree = inspect_fixture(scenario, Some("w0.0")).unwrap();
        assert_eq!(
            subtree["nodes"][0]["fingerprint"],
            snapshot["nodes"][1]["fingerprint"]
        );
    }
}

#[test]
fn app_roots_reject_failed_reads_ambiguous_identity_and_locked_sessions() {
    for (scenario, message) in [
        ("ambiguous", "root identity is ambiguous"),
        ("failed-read", "app AX read failed"),
        ("failed-value", "app AX node read failed"),
        ("failed-role", "app AX node read failed for AXRole"),
        ("changed", "app context changed"),
        ("locked", "desktop is locked"),
    ] {
        let error = inspect_fixture(scenario, Some("w0")).unwrap_err();
        assert!(error.contains(message), "{scenario}: {error}");
    }
}

#[test]
fn partial_nodes_report_unavailable_attributes_and_cannot_issue_action_targets() {
    for (scenario, attribute) in [
        ("unavailable-value", "AXValue"),
        ("unavailable-description", "AXDescription"),
    ] {
        let raw = inspect_fixture(scenario, Some("w0")).unwrap();
        let mut snapshot: AppAccessibilitySnapshot = serde_json::from_value(raw).unwrap();
        issue_targets(&mut snapshot, now().unwrap()).unwrap();
        let child = &snapshot.nodes[1];
        assert_eq!(child.unavailable_attributes, [attribute]);
        if attribute == "AXValue" {
            assert!(child.node.value.is_none());
        }
        assert!(!child.node.settable_value);
        assert!(child.node.actions.is_empty());
        assert!(child.node.target.is_none());
    }
}
