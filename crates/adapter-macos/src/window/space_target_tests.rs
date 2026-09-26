use super::*;
use crate::screenshot::{CapturableWindow, WindowBounds};
use crate::window::{snapshot::SnapshotImage, target::now_seconds};
use serde_json::json;

fn window() -> CapturableWindow {
    CapturableWindow {
        window_id: 42,
        app: "Fixture".into(),
        title: "Fixture window".into(),
        owner_pid: 123,
        is_frontmost: false,
        is_onscreen: false,
        bounds: WindowBounds {
            x: -300,
            y: 20,
            width: 640,
            height: 480,
        },
    }
}

#[test]
fn space_target_binds_identity_membership_and_expiry_without_image_coordinates() {
    let window = WindowIdentity::from(&window());
    let token = MoveTarget::issue(window.clone(), vec![9, 3], 1000).unwrap();
    for now in [1000, 1300] {
        let decoded = MoveTarget::decode(&token, now).unwrap();
        assert_eq!(decoded.window, window);
        assert_eq!(decoded.expected_spaces, Some(vec![3, 9]));
    }
    for now in [999, 1301] {
        assert!(MoveTarget::decode(&token, now).is_err());
    }
    assert!(InputTarget::decode(&token, 1000).is_err());
}

#[test]
fn space_target_accepts_legacy_snapshot_observations_with_their_original_guards() {
    let image = SnapshotImage {
        width: 1280,
        height: 960,
        scale_x: 2.0,
        scale_y: 2.0,
        origin: "window_frame_top_left",
    };
    let token = InputTarget::issue(&window(), &image).unwrap();
    let now = now_seconds().unwrap();
    let decoded = MoveTarget::decode(&token, now).unwrap();
    assert_eq!(decoded.window, WindowIdentity::from(&window()));
    assert!(decoded.expected_spaces.is_none());
    assert!(MoveTarget::decode(&token, now + 301).is_err());
    let mut value: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(token).unwrap()).unwrap();
    value["image_width"] = json!(0);
    let invalid = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&value).unwrap());
    assert!(MoveTarget::decode(&invalid, now).is_err());
}

#[test]
fn space_target_rejects_malformed_identity_membership_and_other_namespaces() {
    let token = MoveTarget::issue(WindowIdentity::from(&window()), vec![3], 1000).unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(token).unwrap()).unwrap();
    for (path, bad) in [
        ("/kind", json!("app_ax")),
        ("/kind", json!("window_input")),
        ("/version", json!(2)),
        ("/issued_at", json!(null)),
        ("/window/window_id", json!(0)),
        ("/window/owner_pid", json!(-1)),
        ("/window/bounds/width", json!(0)),
        ("/window/bounds/height", json!(-1)),
        ("/space_ids", json!([])),
        ("/space_ids", json!([0])),
        ("/space_ids", json!([3, 3])),
        ("/space_ids", json!([9, 3])),
        ("/space_ids", json!([true])),
        ("/space_ids", json!("3")),
        ("/space_ids", json!((1..=65).collect::<Vec<u64>>())),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        let token = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&changed).unwrap());
        assert!(
            MoveTarget::decode(&token, 1000).is_err(),
            "accepted {path}: {changed}"
        );
    }
    for token in ["invalid".to_owned(), "a".repeat(16_385)] {
        assert!(MoveTarget::decode(&token, 1000).is_err());
    }
    for ids in [vec![], vec![0], vec![3, 3]] {
        assert!(MoveTarget::issue(WindowIdentity::from(&window()), ids, 1000).is_err());
    }
}
