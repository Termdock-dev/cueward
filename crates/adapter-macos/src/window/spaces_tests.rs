use super::*;

#[test]
fn space_guards_reject_active_fullscreen_unknown_and_changed_windows() {
    let source = format!(
        "{}\n{}",
        include_str!("space_guards.m"),
        include_str!("space_guards_tests.m")
    );
    let output = run_objc(&source, b"").expect("Space guard fixture");
    assert_eq!(String::from_utf8_lossy(&output).trim(), "passed");
}

#[test]
fn space_commands_reject_missing_identity_before_external_calls() {
    assert!(window_spaces(0).is_err());
    assert!(move_window_to_space("invalid", 0).is_err());
    assert!(move_window_to_space("invalid", 7).is_err());
}

#[test]
fn membership_observation_issues_only_targets_bound_to_the_returned_window() {
    let window: WindowIdentity = serde_json::from_value(json!({
        "window_id": 42, "owner_pid": 123, "title": "Fixture",
        "bounds": {"x": -30, "y": 20, "width": 640, "height": 480}
    }))
    .unwrap();
    let result: WindowSpaces = serde_json::from_value(json!({
        "window_id": 42, "space_ids": [9, 3], "move_target": "untrusted helper data"
    }))
    .unwrap();
    let result = bind_membership(result, window.clone(), 1000).unwrap();
    let public = serde_json::to_value(&result).unwrap();
    let target = MoveTarget::decode(public["move_target"].as_str().unwrap(), 1000).unwrap();
    assert_eq!(target.window, window);
    assert_eq!(target.expected_spaces, Some(vec![3, 9]));
    let empty = serde_json::from_value(json!({"window_id": 42, "space_ids": []})).unwrap();
    let empty = bind_membership(empty, window.clone(), 1000).unwrap();
    assert!(
        serde_json::to_value(empty)
            .unwrap()
            .get("move_target")
            .is_none()
    );
    let wrong = serde_json::from_value(json!({"window_id": 99, "space_ids": [3]})).unwrap();
    assert!(bind_membership(wrong, window, 1000).is_err());
}

fn assert_membership_move_rejections(before: &WindowSpaces, destination: u64) {
    let token = before.move_target.as_deref().expect("move target");
    let identity = MoveTarget::decode(token, now_seconds().unwrap())
        .unwrap()
        .window;
    let held = super::super::input_lock::lock_input(identity.owner_pid).unwrap();
    let busy = move_window_to_space(token, destination).unwrap_err();
    assert!(busy.to_string().contains("another input action"), "{busy}");
    drop(held);
    let wrong = MoveTarget::issue(identity, vec![u64::MAX], now_seconds().unwrap()).unwrap();
    let stale = move_window_to_space(&wrong, destination).unwrap_err();
    assert!(stale.to_string().contains("membership changed"), "{stale}");
    assert_eq!(
        window_spaces(before.window_id).unwrap().space_ids,
        before.space_ids
    );
}

#[test]
#[ignore = "requires a macOS desktop and an existing inactive user Space; no screenshot is taken"]
fn move_disposable_window_with_membership_target_without_capturing() {
    use crate::window::tests::Fixture;
    let catalog = list_spaces().expect("Space catalog");
    let destination = catalog
        .displays
        .iter()
        .flat_map(|d| &d.spaces)
        .find(|s| s.r#type == 0 && !s.is_visible)
        .expect("inactive Space")
        .id;
    let fixture = Fixture::launch_source(include_str!("ax_fixture.swift"), "Cueward AX Fixture");
    let before = window_spaces(fixture.window_id).expect("membership observation");
    assert_ne!(
        before.space_ids,
        vec![destination],
        "fixture must start outside the destination"
    );
    assert_membership_move_rejections(&before, destination);
    let token = before.move_target.as_deref().unwrap();
    let moved = move_window_to_space(token, destination).expect("capture-free move");
    assert_eq!(moved.status, super::super::ActionStatus::Confirmed);
    assert_eq!(moved.before_spaces, before.space_ids);
    assert_eq!(moved.after_spaces, vec![destination]);
    assert!(!moved.foreground_changed && !moved.visible_spaces_changed);
    let after = window_spaces(fixture.window_id).unwrap();
    assert_eq!(after.space_ids, vec![destination]);
    let stale = move_window_to_space(token, destination).unwrap_err();
    let message = stale.to_string();
    assert!(
        message.contains("membership changed") || message.contains("window changed"),
        "{message}"
    );
    let current = after.move_target.as_deref().unwrap();
    assert_eq!(
        move_window_to_space(current, destination).unwrap().status,
        super::super::ActionStatus::Confirmed
    );
}

#[test]
#[ignore = "requires macOS desktop, permissions, and an existing inactive user Space"]
fn move_disposable_window_to_inactive_space_and_read_back_membership() {
    use crate::window::{snapshot_window, tests::Fixture};
    let catalog = list_spaces().expect("Space catalog");
    let destination = catalog
        .displays
        .iter()
        .flat_map(|d| &d.spaces)
        .find(|s| s.r#type == 0 && !s.is_visible)
        .expect("an existing inactive user Space")
        .id;
    assert!(catalog.move_window_available);
    let fixture = Fixture::launch_source(include_str!("ax_fixture.swift"), "Cueward AX Fixture");
    let output = fixture.directory.path().join("window.png");
    let shot = snapshot_window(fixture.window_id, false, output.to_str()).expect("snapshot");
    let original = window_spaces(fixture.window_id)
        .expect("membership")
        .space_ids;
    assert!(!original.is_empty());
    let visible = catalog.displays[0].current_space;
    let rejected =
        move_window_to_space(&shot.input_target, visible).expect_err("active Space rejected");
    assert!(
        rejected.to_string().contains("inactive user Space"),
        "{rejected}"
    );
    let lock = super::super::input_lock::lock_input(shot.window.owner_pid).expect("own lock");
    let busy = move_window_to_space(&shot.input_target, destination)
        .expect_err("input lock must exclude move");
    assert!(busy.to_string().contains("another input action"), "{busy}");
    drop(lock);
    assert_eq!(
        window_spaces(fixture.window_id)
            .expect("still original")
            .space_ids,
        original
    );
    let moved = move_window_to_space(&shot.input_target, destination).expect("move");
    assert_eq!(moved.status, super::super::ActionStatus::Confirmed);
    assert_eq!(moved.before_spaces, original);
    assert_eq!(moved.after_spaces, vec![destination]);
    assert!(!moved.foreground_changed);
    assert!(!moved.visible_spaces_changed);
    assert_eq!(
        window_spaces(fixture.window_id)
            .expect("new membership")
            .space_ids,
        vec![destination]
    );
    snapshot_window(fixture.window_id, false, output.to_str())
        .expect("offscreen snapshot after move");
}
