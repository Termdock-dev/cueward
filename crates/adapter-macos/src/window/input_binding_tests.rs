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

fn status(binary: &Path, directory: &Path, scenario: &str) -> Value {
    let pid = std::process::id();
    let payload = json!({
        "action": "status", "issued_at": super::now_seconds().expect("clock"),
        "caller_pid": pid, "lock_path": directory.join("input.lock"),
    });
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
        .write_all(&serde_json::to_vec(&payload).expect("request"))
        .expect("send request");
    let output = child.wait_with_output().expect("status output");
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
