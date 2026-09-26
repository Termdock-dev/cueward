use super::*;
use crate::window::{ActionStatus, tests::Fixture};

struct CreatedDesktop(u64);

impl Drop for CreatedDesktop {
    fn drop(&mut self) {
        let request = serde_json::to_vec(&json!({"space_id":self.0})).unwrap();
        if let Err(error) = run_objc(include_str!("space_create_cleanup_fixture.m"), &request) {
            eprintln!("test Space {} was not removed: {error}", self.0);
        }
    }
}

#[test]
#[ignore = "requires unlocked macOS desktop and permissions; creates and removes an owned test desktop"]
fn newly_created_inactive_desktop_accepts_a_disposable_window() {
    let before = list_spaces().expect("Space catalog");
    assert!(before.create_space_available);
    let result = create_space().expect("create desktop");
    assert_eq!(result.status, ActionStatus::Confirmed, "{result:?}");
    let id = result.space_id.expect("new Space ID");
    assert!(
        !before
            .displays
            .iter()
            .flat_map(|d| &d.spaces)
            .any(|s| s.id == id)
    );
    let owned = CreatedDesktop(id);
    assert!(result.display_id.is_some());
    assert!(!result.foreground_changed);
    assert_eq!(result.visible_spaces_changed, Some(false));
    {
        let fixture =
            Fixture::launch_source(include_str!("ax_fixture.swift"), "Cueward AX Fixture");
        let membership = window_spaces(fixture.window_id).expect("membership");
        let moved = move_window_to_space(membership.move_target.as_deref().unwrap(), owned.0)
            .expect("move into new desktop");
        assert_eq!(moved.status, ActionStatus::Confirmed);
        assert_eq!(moved.after_spaces, vec![owned.0]);
        assert!(!moved.foreground_changed && !moved.visible_spaces_changed);
        let output = fixture.directory.path().join("created-desktop.png");
        crate::window::snapshot_window(fixture.window_id, false, output.to_str())
            .expect("snapshot");
    }
    drop(owned);
    assert!(
        !list_spaces()
            .unwrap()
            .displays
            .iter()
            .flat_map(|d| &d.spaces)
            .any(|s| s.id == id)
    );
}
