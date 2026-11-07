use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

fn compile_status(directory: &Path) -> std::path::PathBuf {
    let source = directory.join("status.swift");
    let binary = directory.join("status");
    fs::write(
        &source,
        [
            super::super::bridge::AX_SUPPORT,
            include_str!("ax_common.swift"),
            include_str!("input_binding_fixture.swift"),
            include_str!("background_input.swift"),
        ]
        .join("\n"),
    )
    .expect("write status fixture");
    let output = Command::new("swiftc")
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile status helper");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

fn helper(binary: &Path, scenario: &str, request: &Value) -> std::process::Output {
    let pid = std::process::id();
    let mut child = Command::new(binary)
        .args([
            pid.to_string(),
            "42".into(),
            "Fixture dialog".into(),
            "100".into(),
            "100".into(),
            "400".into(),
            "300".into(),
        ])
        .env("INPUT_BINDING_SCENARIO", scenario)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("status helper");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&serde_json::to_vec(request).expect("request"))
        .expect("send request");
    child.wait_with_output().expect("helper output")
}

fn request(action: &str, directory: &Path) -> Value {
    json!({
        "action": action, "issued_at": super::now_seconds().expect("clock"),
        "caller_pid": std::process::id(), "lock_path": directory.join("input.lock"),
        "x": 40.0, "y": 40.0, "to_x": 60.0, "to_y": 60.0, "button": "left", "count": 1,
        "duration_ms": 100, "delta_x": 0, "delta_y": -40,
    })
}

fn status(binary: &Path, directory: &Path, scenario: &str) -> Value {
    let output = helper(binary, scenario, &request("status", directory));
    assert!(
        output.status.success(),
        "{scenario}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("status JSON")
}

#[test]
fn keyboard_status_binds_native_identity_and_preserves_legacy_fallback() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let binary = compile_status(directory.path());
    let cases = [
        ("title-missing", true),
        ("title-different", true),
        ("valid", true),
        ("wrong-id", false),
        ("lookup-error", false),
        ("zero-id", false),
        ("wrong-owner", false),
        ("owner-error", false),
        ("wrong-bounds", false),
        ("missing-focused", false),
        ("malformed-focused", false),
        ("legacy", true),
        ("legacy-missing-title", false),
        ("legacy-ambiguous", false),
    ];
    let mut failures = Vec::new();
    for (scenario, ready) in cases {
        let observed = status(&binary, directory.path(), scenario);
        if observed["keyboard"]["dispatch_ready"] != ready {
            failures.push(format!("{scenario}: {observed}"));
        }
        assert_eq!(observed["pointer"]["dispatch_ready"], true, "{scenario}");
        assert_eq!(observed["application_acceptance"], "unverified");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn pointer_input_refuses_a_frame_that_is_only_a_thumbnail() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let binary = compile_status(directory.path());
    let observed = status(&binary, directory.path(), "thumbnail");
    assert_eq!(observed["pointer"]["dispatch_ready"], false, "{observed}");
    assert!(
        observed["pointer"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("thumbnail")),
        "{observed}"
    );
    for action in ["click", "drag", "scroll"] {
        let output = helper(&binary, "thumbnail", &request(action, directory.path()));
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{action}: {text}");
        assert!(text.contains("thumbnail"), "{action}: {text}");
    }
}
