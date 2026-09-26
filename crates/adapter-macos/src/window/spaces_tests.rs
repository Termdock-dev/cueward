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
