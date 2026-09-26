use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use super::*;

fn token() -> String {
    URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&json!({
            "kind": "window_input", "version": 1, "issued_at": 1000,
            "window": {"window_id": 42, "owner_pid": 123, "title": "Fixture",
                "bounds": {"x": -800, "y": 40, "width": 400, "height": 300}},
            "image_width": 800, "image_height": 600,
        }))
        .expect("JSON"),
    )
}

#[test]
fn input_target_binds_snapshot_coordinates_and_expires() {
    let target = InputTarget::decode(&token(), 1000).expect("target");
    assert_eq!(
        target.frame_point(200.0, 100.0).expect("point"),
        (100.0, 50.0)
    );
    for point in [
        (f64::NAN, 0.0),
        (0.0, f64::INFINITY),
        (-1.0, 0.0),
        (800.0, 0.0),
        (0.0, 600.0),
    ] {
        assert!(target.frame_point(point.0, point.1).is_err());
    }
    assert!(InputTarget::decode(&token(), 1300).is_ok());
    assert!(InputTarget::decode(&token(), 1301).is_err());
    assert!(InputTarget::decode(&token(), 999).is_err());
    assert!(InputTarget::decode("invalid!", 1000).is_err());
    let mut payload: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(token()).expect("base64")).expect("JSON");
    for (field, value) in [
        ("kind", json!("AX")),
        ("image_width", json!(0)),
        ("image_height", json!(u32::MAX)),
    ] {
        let original = payload[field].clone();
        payload[field] = value;
        let invalid = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).expect("JSON"));
        assert!(InputTarget::decode(&invalid, 1000).is_err());
        payload[field] = original;
    }
}

#[test]
fn input_validation_rejects_unbounded_or_unsupported_requests() {
    for text in ["", "line\nbreak", "a\tb", "\0"] {
        assert!(
            type_text("unused", text)
                .expect_err("invalid text")
                .to_string()
                .contains("text must contain")
        );
    }
    assert!(type_text("unused", &"a".repeat(1025)).is_err());
    assert_eq!(key_code("tab"), Some(48));
    assert_eq!(key_code("enter"), Some(36));
    assert!(key_code("invented").is_none());
    assert_eq!(
        modifier_flags(&["command".into(), "shift".into()]).expect("flags"),
        (1 << 20) | (1 << 17)
    );
    assert!(modifier_flags(&["caps-lock".into()]).is_err());
    for delta in [(0, 0), (i32::MIN, 0), (0, 4097)] {
        assert!(
            scroll("unused", 0.0, 0.0, delta.0, delta.1)
                .expect_err("invalid deltas")
                .to_string()
                .contains("scroll deltas")
        );
    }
}
