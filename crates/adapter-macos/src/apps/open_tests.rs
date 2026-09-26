use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

#[test]
fn open_request_requires_explicit_app_and_absolute_regular_file() {
    let directory = tempfile::tempdir().expect("files");
    let file = directory.path().join("測試 file.txt");
    fs::write(&file, "fixture").expect("file");
    assert!(open_request(None, None, &file, false).is_err());
    for invalid in [
        Path::new("relative.txt"),
        directory.path(),
        &directory.path().join("missing"),
    ] {
        assert!(open_request(Some("org.example.App"), None, invalid, false).is_err());
    }
    let link = directory.path().join("link.txt");
    symlink(&file, &link).expect("symlink");
    let request = open_request(Some("org.example.App"), None, &link, true).expect("request");
    assert_eq!(
        request["file"],
        file.canonicalize()
            .expect("canonical")
            .to_str()
            .expect("UTF-8")
    );
    assert_eq!(request["action"], "open");
    assert_eq!(request["new_instance"], true);
}

fn fixture_app(directory: &Path) -> std::path::PathBuf {
    let app = directory.join("Fixture.app");
    let contents = app.join("Contents");
    fs::create_dir_all(contents.join("MacOS")).expect("bundle");
    fs::write(
        contents.join("Info.plist"),
        r#"<?xml version="1.0"?><plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>org.example.openfixture</string>
<key>CFBundleExecutable</key><string>Fixture</string>
<key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#,
    )
    .expect("metadata");
    let executable = contents.join("MacOS/Fixture");
    fs::write(&executable, "#!/bin/sh\nexit 0\n").expect("executable");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).expect("mode");
    app
}

fn compile_fixture(directory: &Path) -> std::path::PathBuf {
    let source = directory.join("open.swift");
    fs::write(
        &source,
        format!(
            "{}\n{}",
            include_str!("open_fixture.swift"),
            helper_source()
        ),
    )
    .expect("source");
    let binary = directory.join("open");
    let output = Command::new("swiftc")
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("swiftc");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

fn observe_case(
    binary: &Path,
    app: &Path,
    directory: &Path,
    scenario: &str,
    file: &Path,
) -> (std::process::Output, Option<Value>) {
    let trace = directory.join("dispatch.json");
    if trace.exists() {
        fs::remove_file(&trace).expect("remove prior trace");
    }
    let mut request = launch_request(None, Some(app)).expect("selector");
    request["action"] = json!("open");
    request["file"] = json!(file);
    request["new_instance"] = json!(scenario.starts_with("new-"));
    request["lock_dir"] = json!(directory);
    let output = run_with_timeout(
        Command::new(binary)
            .env("OPEN_SCENARIO", scenario)
            .env("OPEN_APP", app)
            .env("OPEN_TRACE", &trace),
        &serde_json::to_vec(&request).expect("JSON"),
        Duration::from_secs(10),
    )
    .expect("helper");
    let dispatch = fs::read(trace)
        .ok()
        .map(|v| serde_json::from_slice(&v).expect("dispatch JSON"));
    (output, dispatch)
}

#[test]
fn open_helper_validates_recipient_and_dispatches_conservative_workspace_configuration() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let app = fixture_app(directory.path());
    let binary = compile_fixture(directory.path());
    let file = directory.path().join("input 測試.txt");
    fs::write(&file, "fixture content").expect("file");
    for scenario in [
        "cold",
        "new-app",
        "existing",
        "new-instance",
        "new-foreground",
        "foreground-changed",
    ] {
        let (output, dispatch) = observe_case(&binary, &app, directory.path(), scenario, &file);
        check_success(output, dispatch, scenario, &file);
    }
    check_rejections(&binary, &app, directory.path(), &file);
}

fn check_success(
    output: std::process::Output,
    dispatch: Option<Value>,
    scenario: &str,
    file: &Path,
) {
    assert!(
        output.status.success(),
        "{scenario}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let observed: OpenFileResult = serde_json::from_slice(&output.stdout).expect("open result");
    assert_eq!(observed.status, OpenFileStatus::SentUnverified);
    assert_eq!(
        Path::new(&observed.file)
            .canonicalize()
            .expect("returned file"),
        file.canonicalize().expect("canonical")
    );
    assert_eq!(
        observed.app.pid,
        if scenario == "existing" { 100 } else { 201 }
    );
    assert_eq!(
        observed.foreground_changed,
        scenario == "foreground-changed"
    );
    let dispatch = dispatch.expect("one open request");
    assert_eq!(dispatch["calls"], 1);
    assert_eq!(dispatch["files"], json!([observed.file]));
    assert_eq!(dispatch["new_instance"], scenario.starts_with("new-"));
    for flag in [
        "activates",
        "recent_items",
        "hides_others",
        "prompts",
        "substitution",
    ] {
        assert_eq!(dispatch[flag], false, "{scenario}: {flag}");
    }
}

fn check_rejections(binary: &Path, app: &Path, directory: &Path, file: &Path) {
    for (scenario, submitted, expected) in [
        ("foreground", false, "foreground"),
        ("foreground-missing", false, "foreground"),
        ("foreground-zero", false, "foreground"),
        ("ambiguous", false, "multiple application instances"),
        ("other-path", false, "another path"),
        ("dead-caller", false, "caller exited"),
        ("lock-busy", false, "another input action"),
        ("post-lock-recipient", false, "recipient changed before"),
        ("new-reused", true, "existing process"),
        ("recipient-changed", true, "recipient changed"),
        ("callback-error", true, "document open failed"),
        ("callback-empty", true, "document open failed"),
        ("callback-other-path", true, "path differs"),
        ("terminated-result", true, "exited"),
    ] {
        let (output, dispatch) = observe_case(binary, app, directory, scenario, file);
        assert!(!output.status.success(), "accepted {scenario}");
        assert_eq!(dispatch.is_some(), submitted, "{scenario}");
        if let Some(dispatch) = dispatch {
            assert_eq!(dispatch["calls"], 1);
        }
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{scenario}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for invalid in [
        directory.to_path_buf(),
        directory.join("missing.txt"),
        Path::new("relative.txt").to_path_buf(),
    ] {
        let (output, dispatch) = observe_case(binary, app, directory, "existing", &invalid);
        assert!(!output.status.success());
        assert!(dispatch.is_none());
    }
}
