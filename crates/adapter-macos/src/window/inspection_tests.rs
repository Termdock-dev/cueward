use std::io::Write;
use std::process::Command;
use std::time::Duration;

use super::*;
use crate::window::AccessibilityNode;

#[test]
fn root_validation_rejects_ambiguous_and_unbounded_paths_before_inspection() {
    assert_eq!(root_depth("0").expect("root"), 0);
    assert_eq!(root_depth("0.2.17").expect("child"), 2);
    assert_eq!(
        root_depth(&format!("0{}", ".1".repeat(12))).expect("deepest"),
        12
    );
    for root in [
        "",
        "1",
        "0.",
        "0..1",
        "0.-1",
        "0.+1",
        "0.01",
        "0.text",
        "0.9223372036854775808",
    ] {
        assert!(
            inspect_window_subtree(0, root, 10, 2, false, false).is_err(),
            "{root}"
        );
        assert!(root_depth(root).is_err(), "{root}");
    }
    assert!(root_depth(&format!("0{}", ".1".repeat(13))).is_err());
    for (limit, depth) in [(0, 1), (501, 1), (1, 0), (1, 13)] {
        assert!(inspect_window_subtree(0, "0", limit, depth, false, false).is_err());
    }
}

#[test]
fn nodes_preserve_fractional_screen_bounds_and_optional_geometry() {
    let mut payload = serde_json::json!({
        "ref": "0.1", "role": "AXGroup", "name": "Editor", "actions": [],
        "settable_value": false, "fingerprint": "internal", "child_count": 2,
        "bounds": {"x": -800.25, "y": 40.5, "width": 300.75, "height": 120.0}
    });
    let node: AccessibilityNode = serde_json::from_value(payload.clone()).expect("node");
    assert_eq!(node.bounds.as_ref().expect("bounds").x, -800.25);
    assert_eq!(node.child_count, 2);
    let public = serde_json::to_value(node).expect("public node");
    assert!(public.get("fingerprint").is_none());
    assert_eq!(public["bounds"]["width"], 300.75);
    payload.as_object_mut().expect("object").remove("bounds");
    let node: AccessibilityNode = serde_json::from_value(payload).expect("no geometry");
    assert!(node.bounds.is_none());
    assert!(
        serde_json::to_value(node)
            .expect("node")
            .get("bounds")
            .is_none()
    );
}

fn inspect_fixture(root: &str, limit: usize, depth: usize) -> std::process::Output {
    let mut script = tempfile::NamedTempFile::with_suffix(".swift").expect("script");
    write!(
        script,
        "{}\n{}",
        include_str!("ax_inspection_fixture.swift"),
        include_str!("ax_inspect.swift")
    )
    .expect("write script");
    super::super::process::run_with_timeout(
        Command::new("swift")
            .arg(script.path())
            .args(["unused"; 7])
            .args([&limit.to_string(), &depth.to_string(), root]),
        b"",
        Duration::from_secs(40),
    )
    .expect("run traversal fixture")
}

fn inspect(root: &str, limit: usize, depth: usize) -> serde_json::Value {
    let output = inspect_fixture(root, limit, depth);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("inspection JSON")
}

#[test]
fn executable_traversal_explores_subtrees_with_absolute_refs_and_ancestor_identity() {
    let all = inspect("0", 20, 12);
    let subtree = inspect("0.0", 20, 12);
    let nodes = subtree["nodes"].as_array().expect("nodes");
    assert_eq!(subtree["root_ref"], "0.0");
    assert_eq!(subtree["truncated"], false);
    assert_eq!(
        nodes
            .iter()
            .map(|n| n["ref"].as_str().expect("ref"))
            .collect::<Vec<_>>(),
        ["0.0", "0.0.0", "0.0.1"]
    );
    assert_eq!(nodes[0]["parent_ref"], "0");
    assert_eq!(nodes[0]["child_count"], 2);
    assert_eq!(nodes[1]["child_count"], 0);
    for node in nodes {
        let original = all["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .find(|n| n["ref"] == node["ref"])
            .expect("same ref");
        assert_eq!(node["fingerprint"], original["fingerprint"]);
    }
}

#[test]
fn executable_traversal_reports_limits_and_rejects_missing_roots() {
    let shallow = inspect("0", 20, 1);
    assert_eq!(shallow["truncated"], true);
    assert_eq!(shallow["nodes"].as_array().expect("nodes").len(), 3);
    let limited = inspect("0.0", 1, 12);
    assert_eq!(limited["truncated"], true);
    assert_eq!(limited["nodes"].as_array().expect("nodes").len(), 1);
    let leaf = inspect("0.0.0", 1, 0);
    assert_eq!(leaf["truncated"], false);
    let capped = inspect("0.0", 20, 0);
    assert_eq!(capped["truncated"], true);
    let missing = inspect_fixture("0.4", 20, 12);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("root disappeared"));
}
