use super::*;
use std::path::Path;

fn compile_fixture(directory: &Path) -> std::path::PathBuf {
    let source = directory.join("creation.m");
    fs::write(
        &source,
        format!(
            "{}\n{}\n{}\n{}",
            include_str!("space_guards.m"),
            include_str!("space_create_fixture.m"),
            include_str!("space_create.m"),
            r#"int main(void) { @autoreleasepool {
                request = [NSJSONSerialization JSONObjectWithData:NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile options:0 error:nil];
                scenario = request[@"scenario"]; createSpace(request); return 0;
            } }"#
        ),
    )
    .unwrap();
    let binary = directory.join("creation");
    let output = Command::new("clang")
        .args(["-fobjc-arc", "-framework", "Cocoa"])
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

fn observe(binary: &Path, directory: &Path, scenario: &str) -> (std::process::Output, Value) {
    let trace = directory.join("trace.json");
    let payload = json!({"scenario":scenario,"trace":trace,
        "lock_path":directory.join("creation.lock"),"caller_pid":std::process::id()});
    let output = run_with_timeout(
        &mut Command::new(binary),
        &serde_json::to_vec(&payload).unwrap(),
        Duration::from_secs(8),
    )
    .unwrap();
    let trace = serde_json::from_slice(&fs::read(trace).unwrap()).unwrap();
    (output, trace)
}

#[test]
fn creation_dispatches_once_and_confirms_only_its_new_inactive_desktop() {
    let directory = tempfile::tempdir().unwrap();
    let binary = compile_fixture(directory.path());
    for scenario in ["normal", "delayed", "foreground-changed"] {
        let (output, trace) = observe(&binary, directory.path(), scenario);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: SpaceCreateResult = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result.status, super::super::super::ActionStatus::Confirmed);
        assert_eq!(result.space_id, Some(99));
        assert_eq!(result.display_id.as_deref(), Some("fixture-display"));
        assert!(result.reason.is_none());
        assert_eq!(result.visible_spaces_before, vec![1]);
        assert_eq!(result.visible_spaces_after, Some(vec![1]));
        assert_eq!(result.visible_spaces_changed, Some(false));
        assert_eq!(result.foreground_changed, scenario == "foreground-changed");
        assert_eq!(trace["calls"], 1);
        assert_eq!(trace["options"], 0);
        assert_eq!(trace["values"]["type"], 0);
        assert_eq!(trace["values"]["uuid"].as_str().unwrap().len(), 36);
    }
    assert_rejections(&binary, directory.path());
    assert_uncertain(&binary, directory.path());
}

fn assert_rejections(binary: &Path, directory: &Path) {
    for (scenario, message) in [
        ("unavailable", "unavailable"),
        ("missing-before", "catalog"),
        ("no-foreground", "foreground"),
        ("dead-caller", "caller exited"),
        ("caller-exited", "before submission"),
        ("prepare-failed", "cannot prepare"),
    ] {
        let (output, trace) = observe(binary, directory, scenario);
        assert!(!output.status.success(), "{scenario}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{scenario}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(trace["calls"], 0, "{scenario}");
    }
    let _lock = crate::window::input_lock::lock_path(&directory.join("creation.lock")).unwrap();
    let (output, trace) = observe(binary, directory, "normal");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("another Space creation"));
    assert_eq!(trace["calls"], 0);
}

fn assert_uncertain(binary: &Path, directory: &Path) {
    for scenario in [
        "zero",
        "reused",
        "missing-result",
        "absent",
        "wrong-uuid",
        "wrong-type",
        "visible",
        "duplicate",
        "missing-after",
    ] {
        let (output, trace) = observe(binary, directory, scenario);
        assert!(
            output.status.success(),
            "{scenario}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: SpaceCreateResult = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            result.status,
            super::super::super::ActionStatus::SentUnverified,
            "{scenario}"
        );
        assert!(result.display_id.is_none() && result.reason.is_some());
        assert_eq!(
            result.space_id,
            if ["zero", "reused", "missing-result"].contains(&scenario) {
                None
            } else {
                Some(99)
            }
        );
        assert_eq!(trace["calls"], 1, "must not retry {scenario}");
        if scenario == "missing-after" {
            assert!(
                result.visible_spaces_after.is_none() && result.visible_spaces_changed.is_none()
            );
        } else {
            assert_eq!(result.visible_spaces_changed, Some(scenario == "visible"));
        }
    }
}
