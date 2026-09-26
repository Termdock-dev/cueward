use super::*;

fn target() -> Target {
    Target {
        kind: "app_ax".into(),
        version: 1,
        issued_at: 100,
        app: AppInstance {
            pid: 123,
            start_seconds: 50,
            start_micros: 9,
            executable: "/Applications/Fixture.app/Contents/MacOS/Fixture".into(),
        },
        r#ref: "w0.2.1".into(),
        root_fingerprint: "a".repeat(64),
        context: "b".repeat(64),
        fingerprint: "c".repeat(64),
    }
}

#[test]
fn app_target_roundtrip_and_expiry() {
    let token = target().encode().unwrap();
    let decoded = Target::decode(&token, 400).unwrap();
    assert_eq!(decoded.app, target().app);
    assert_eq!(decoded.r#ref, "w0.2.1");
    assert!(Target::decode(&token, 401).is_err());
    assert!(Target::decode(&token, 99).is_err());
    for input in [
        "not-a-token".into(),
        "a".repeat(16_385),
        URL_SAFE_NO_PAD.encode(b"{}"),
    ] {
        assert!(Target::decode(&input, 100).is_err());
    }
}

#[test]
fn app_targets_reject_malformed_or_other_namespace_bindings() {
    let mutations: &[fn(&mut Target)] = &[
        |t| t.kind = "ax".into(),
        |t| t.version = 2,
        |t| t.app.pid = 0,
        |t| t.app.start_seconds = 0,
        |t| t.app.start_micros = 1_000_000,
        |t| t.app.executable = "relative/app".into(),
        |t| t.r#ref = "0.1".into(),
        |t| t.context.clear(),
        |t| t.root_fingerprint = "z".repeat(64),
        |t| t.fingerprint = "a".repeat(63),
    ];
    for mutate in mutations {
        let mut value = target();
        mutate(&mut value);
        assert!(Target::decode(&value.encode().unwrap(), 100).is_err());
    }
}

#[test]
fn app_refs_are_bounded_and_unambiguous() {
    for (reference, depth) in [("menu", 0), ("w0", 0), ("w63.1.20", 2), ("menu.0.2", 2)] {
        assert_eq!(ref_depth(reference).unwrap(), depth);
    }
    assert_eq!(ref_depth(&format!("w1{}", ".1".repeat(12))).unwrap(), 12);
    for reference in [
        "",
        "w64",
        "w01",
        "menu.",
        "w0.-1",
        "w0.01",
        "w0.１",
        "w0.999999999999999999999",
    ] {
        assert!(ref_depth(reference).is_err(), "{reference}");
    }
    assert!(ref_depth(&format!("menu{}", ".1".repeat(13))).is_err());
}
