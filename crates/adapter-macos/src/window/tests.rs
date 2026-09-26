use std::fs;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use super::target::{Target, now_seconds};
use super::{
    ActionStatus, WindowInspectResult, inspect_window, inspect_window_subtree, press, set_value,
};
use crate::screenshot::list_capturable_windows;

struct Fixture {
    child: Child,
    directory: tempfile::TempDir,
    window_id: u32,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Fixture {
    fn launch() -> Self {
        let directory = tempfile::tempdir().expect("temporary fixture directory");
        let source = directory.path().join("ax_fixture.swift");
        let executable = directory.path().join("CuewardAXFixture");
        fs::write(&source, include_str!("ax_fixture.swift")).expect("write fixture");
        let compile = Command::new("swiftc")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("compile fixture");
        assert!(
            compile.status.success(),
            "fixture compilation failed: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let child = Command::new(&executable)
            .arg(directory.path().join("state.json"))
            .spawn()
            .expect("launch fixture");
        let mut fixture = Self {
            child,
            directory,
            window_id: 0,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let window = list_capturable_windows()
                .expect("list windows")
                .into_iter()
                .find(|window| {
                    window.owner_pid == fixture.child.id() as i32
                        && window.title == "Cueward AX Fixture"
                });
            if let Some(window) = window {
                fixture.window_id = window.window_id;
                return fixture;
            }
            assert!(Instant::now() < deadline, "fixture window did not appear");
            thread::sleep(Duration::from_millis(200));
        }
    }

    fn inspect(&self) -> WindowInspectResult {
        inspect_window(self.window_id, 100, 6, false, false).expect("inspect fixture")
    }

    fn wait_for_state(&self, count: u64, value: &str) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let state: serde_json::Value = serde_json::from_slice(
                &fs::read(self.directory.path().join("state.json")).expect("read fixture state"),
            )
            .expect("fixture JSON");
            if state["count"] == count && state["text"] == value {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "unexpected fixture state: {state}"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }
}

fn target(result: &WindowInspectResult, name: &str) -> String {
    result
        .accessibility
        .nodes
        .iter()
        .find(|node| node.name == name)
        .and_then(|node| node.target.clone())
        .unwrap_or_else(|| panic!("missing target for {name}"))
}

fn check_inspection(fixture: &Fixture, result: &WindowInspectResult) {
    assert_eq!(result.window.owner_pid, fixture.child.id() as i32);
    assert!(!result.accessibility.truncated);
    assert_eq!(result.accessibility.root_ref, "0");
    assert!(
        result
            .accessibility
            .nodes
            .iter()
            .any(|node| node.value.as_deref() == Some("ready"))
    );
    let secret = result
        .accessibility
        .nodes
        .iter()
        .find(|node| node.subrole.as_deref() == Some("AXSecureTextField"))
        .expect("secure field is present");
    assert!(secret.value.is_none());
    assert!(!secret.settable_value);
    assert!(secret.target.is_none());
    let limited =
        inspect_window(fixture.window_id, 1, 6, false, false).expect("limited inspection");
    assert_eq!(limited.accessibility.nodes.len(), 1);
    assert!(limited.accessibility.truncated);
}

#[test]
#[ignore = "requires a logged-in macOS desktop and Accessibility permission"]
fn window_subtree_inspection_preserves_action_targets() {
    let fixture = Fixture::launch();
    let overview = inspect_window(fixture.window_id, 100, 1, false, false).expect("shallow view");
    assert!(overview.accessibility.truncated);
    let group = overview
        .accessibility
        .nodes
        .iter()
        .find(|node| node.role == "AXGroup" && node.name == "Editor")
        .expect("discover editor group");
    assert!(group.child_count > 0);
    let frame = group.bounds.as_ref().expect("group geometry");
    assert!(frame.width > 0.0 && frame.height > 0.0);
    let detail = inspect_window_subtree(fixture.window_id, &group.r#ref, 100, 6, false, false)
        .expect("explore group");
    assert_eq!(detail.accessibility.root_ref, group.r#ref);
    assert!(!detail.accessibility.truncated);
    assert!(
        detail
            .accessibility
            .nodes
            .iter()
            .all(|node| node.r#ref == group.r#ref
                || node.r#ref.starts_with(&format!("{}.", group.r#ref)))
    );
    let full = fixture.inspect();
    for node in &detail.accessibility.nodes {
        let original = full
            .accessibility
            .nodes
            .iter()
            .find(|n| n.r#ref == node.r#ref)
            .expect("absolute ref");
        assert_eq!(node.fingerprint, original.fingerprint);
    }
    press(&target(&detail, "Increment")).expect("press a subtree target");
    fixture.wait_for_state(1, "draft");
    let fresh = inspect_window_subtree(fixture.window_id, &group.r#ref, 100, 6, false, false)
        .expect("observe after action");
    assert!(
        fresh
            .accessibility
            .nodes
            .iter()
            .any(|node| node.value.as_deref() == Some("pressed 1"))
    );
    let result = set_value(&target(&fresh, "Draft"), "explored").expect("set a subtree target");
    assert_eq!(result.status, ActionStatus::Confirmed);
    fixture.wait_for_state(1, "explored");
}

#[test]
#[ignore = "requires a logged-in macOS desktop and Accessibility permission"]
fn window_inspection_and_background_actions_work_on_real_app() {
    let fixture = Fixture::launch();
    let result = fixture.inspect();
    check_inspection(&fixture, &result);
    let pressed = press(&target(&result, "Increment")).expect("press fixture button");
    assert_eq!(pressed.status, ActionStatus::SentUnverified);
    assert!(!pressed.foreground_changed);
    assert_ne!(pressed.frontmost_pid_before, fixture.child.id() as i32);
    fixture.wait_for_state(1, "draft");

    let no_effect = press(&target(&fixture.inspect(), "No effect")).expect("press no-op button");
    assert_eq!(no_effect.status, ActionStatus::SentUnverified);
    assert!(!no_effect.foreground_changed);
    fixture.wait_for_state(1, "draft");

    let input = target(&fixture.inspect(), "Draft");
    let text = "edited 測試 \"quotes\" \\ text";
    let changed = set_value(&input, text).expect("set fixture text");
    assert_eq!(changed.status, ActionStatus::Confirmed);
    assert!(!changed.foreground_changed);
    fixture.wait_for_state(1, text);
    let stale = set_value(&input, "wrong").expect_err("stale element must be rejected");
    assert!(stale.to_string().contains("element changed"), "{stale}");
    fixture.wait_for_state(1, text);

    let mut wrong = Target::decode(&input, now_seconds().expect("clock")).expect("decode target");
    wrong.window.title = "different window".into();
    let error = set_value(&wrong.encode().expect("token"), "wrong")
        .expect_err("window change must be rejected");
    assert!(
        error.to_string().contains("window identity changed"),
        "{error}"
    );
    fixture.wait_for_state(1, text);
}
