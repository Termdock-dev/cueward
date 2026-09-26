use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use super::tests::Fixture;
use super::{AccessibilitySurface, WindowInspectResult, inspect_window_surface, press, set_value};

fn menu(fixture: &Fixture, root: &str) -> WindowInspectResult {
    inspect_window_surface(
        fixture.window_id,
        AccessibilitySurface::Menu,
        root,
        200,
        8,
        false,
        false,
    )
    .expect("menu inspection")
}

fn target(result: &WindowInspectResult, name: &str) -> String {
    result
        .accessibility
        .nodes
        .iter()
        .find(|n| n.name == name)
        .and_then(|n| n.target.clone())
        .unwrap_or_else(|| panic!("missing target: {name}"))
}

fn wait_state(fixture: &Fixture, predicate: impl Fn(&serde_json::Value) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let state: serde_json::Value = serde_json::from_slice(
            &fs::read(fixture.directory.path().join("state.json")).expect("state"),
        )
        .expect("JSON");
        assert_eq!(state["active"], false);
        if predicate(&state) {
            return;
        }
        assert!(Instant::now() < deadline, "unexpected state: {state}");
        thread::sleep(Duration::from_millis(30));
    }
}

#[test]
#[ignore = "requires a logged-in macOS desktop and Accessibility permission"]
fn menu_exploration_executes_leaf_actions_and_discovers_standard_sheets() {
    let fixture =
        Fixture::launch_source(include_str!("menu_fixture.swift"), "Cueward Menu Fixture");
    let overview = menu(&fixture, "0");
    assert_eq!(overview.accessibility.surface, AccessibilitySurface::Menu);
    assert!(
        overview
            .accessibility
            .nodes
            .iter()
            .filter(|n| n.role != "AXMenuItem" || n.child_count > 0)
            .all(|n| n.target.is_none())
    );
    let item = overview
        .accessibility
        .nodes
        .iter()
        .find(|n| n.name == "Increment")
        .expect("item");
    let detail = menu(&fixture, &item.r#ref);
    press(&target(&detail, "Increment")).expect("background menu action");
    wait_state(&fixture, |s| s["count"] == 1);
    press(&target(&menu(&fixture, "0"), "Save Document")).expect("open sheet");
    wait_state(&fixture, |s| s["sheet"].as_u64().is_some_and(|id| id > 0));
    let observed = fixture.inspect();
    assert!(
        observed
            .accessibility
            .nodes
            .iter()
            .any(|n| n.role == "AXSheet")
    );
    for node in &observed.accessibility.nodes {
        if node.enabled == Some(false) {
            assert!(node.target.is_none());
        }
    }
    let name = observed
        .accessibility
        .nodes
        .iter()
        .find(|n| n.value.as_deref() == Some("Result.txt"))
        .and_then(|n| n.target.as_deref())
        .expect("filename field");
    let changed = set_value(name, "Edited.txt").expect("edit filename");
    assert_eq!(changed.status, super::ActionStatus::Confirmed);
    press(&target(&fixture.inspect(), "Cancel")).expect("cancel sheet");
    wait_state(&fixture, |s| s["result"] == "cancelled" && s["sheet"] == 0);
    assert!(!fixture.directory.path().join("Edited.txt").exists());
}
