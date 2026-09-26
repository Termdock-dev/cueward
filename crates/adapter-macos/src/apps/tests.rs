use super::*;
use std::thread;
use std::time::Instant;

#[test]
fn launch_selectors_reject_ambiguity_and_invalid_names_before_dispatch() {
    assert!(launch_request(None, None).is_err());
    assert!(launch_request(Some("org.example.App"), Some(Path::new("/tmp/Fixture.app"))).is_err());
    for id in ["", "bad id", "bad\nname", "/tmp/Fixture.app"] {
        assert!(launch_request(Some(id), None).is_err(), "accepted {id:?}");
    }
    for path in ["relative.app", "/tmp/file.txt"] {
        assert!(launch_request(None, Some(Path::new(path))).is_err());
    }
    assert!(launch_request(Some("org.example.App-2"), None).is_ok());
    assert!(launch_request(None, Some(Path::new("/tmp/An App.app"))).is_ok());
}

#[test]
fn launch_rejects_non_application_directories_in_helper() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let path = directory.path().join("Empty.app");
    fs::create_dir(&path).expect("empty app directory");
    let error = launch_app(None, Some(&path)).expect_err("invalid bundle");
    assert!(
        error
            .to_string()
            .contains("not an executable application bundle"),
        "{error}"
    );
}

pub(super) struct Fixture {
    pub(super) directory: tempfile::TempDir,
    pub(super) path: std::path::PathBuf,
}

impl Fixture {
    fn build() -> Self {
        Self::with_source(include_str!("fixture.swift"))
    }

    pub(super) fn with_source(body: &str) -> Self {
        let directory = tempfile::tempdir().expect("app fixture directory");
        let path = directory.path().join("CuewardApplicationFixture.app");
        let contents = path.join("Contents");
        fs::create_dir_all(contents.join("MacOS")).expect("bundle directories");
        let source = directory.path().join("fixture.swift");
        fs::write(&source, body).expect("fixture source");
        let compiled = Command::new("swiftc")
            .arg(source)
            .arg("-o")
            .arg(contents.join("MacOS/Fixture"))
            .output()
            .expect("compile fixture");
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let id = format!("org.example.cueward.fixture.{}", std::process::id());
        fs::write(contents.join("Info.plist"), format!(
            "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>{id}</string><key>CFBundleExecutable</key><string>Fixture</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"
        )).expect("bundle metadata");
        Self { directory, path }
    }

    fn state(&self) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(bytes) = fs::read(self.directory.path().join("state.json")) {
                if let Ok(state) = serde_json::from_slice(&bytes) {
                    return state;
                }
            }
            assert!(Instant::now() < deadline, "fixture state did not appear");
            thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let states = fs::read_dir(self.directory.path())
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name == "state.json"
                            || (name.starts_with("state-") && name.ends_with(".json"))
                    })
            });
        for path in states {
            if let Ok(data) = fs::read(path) {
                if let Ok(value) = serde_json::from_slice::<Value>(&data) {
                    if let Some(pid) = value["pid"].as_i64() {
                        let _ = Command::new("/bin/kill")
                            .arg("-TERM")
                            .arg(pid.to_string())
                            .output();
                    }
                }
            }
        }
        let _ = Command::new("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister")
            .arg("-u").arg(&self.path).output();
    }
}

#[test]
fn launch_helper_has_no_unsafe_cross_queue_capture_diagnostics() {
    let directory = tempfile::tempdir().expect("compiler fixture");
    let source = directory.path().join("apps.swift");
    fs::write(&source, helper_source()).expect("production helper");
    let output = Command::new("swiftc")
        .args(["-strict-concurrency=complete", "-warnings-as-errors"])
        .arg(source)
        .arg("-o")
        .arg(directory.path().join("apps"))
        .output()
        .expect("strict concurrency compilation");
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    // Preconcurrency framework imports can keep this diagnostic a warning even with -warnings-as-errors.
    assert!(
        output.status.success() && !diagnostics.contains("SendableClosureCaptures"),
        "{diagnostics}"
    );
}

#[test]
fn completion_and_timeout_have_one_winner_across_queues() {
    let directory = tempfile::tempdir().expect("completion fixture");
    let source = directory.path().join("completion.swift");
    fs::write(
        &source,
        format!(
            "{}\n{}",
            include_str!("completion.swift"),
            include_str!("completion_tests.swift")
        ),
    )
    .expect("completion tests");
    let output = Command::new("swift")
        .arg(source)
        .output()
        .expect("cross-queue completion test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "passed");
}

#[test]
#[ignore = "uses a controlled workspace catalog; may launch one disposable application on regression"]
fn bundle_id_resolution_rejects_multiple_installed_copies() {
    let first = Fixture::build();
    let second = Fixture::build();
    let source = first.directory.path().join("catalog.swift");
    fs::write(
        &source,
        format!(
            "{}\n{}",
            include_str!("catalog_fixture.swift"),
            helper_source()
        ),
    )
    .expect("controlled catalog and production helper");
    let id = format!("org.example.cueward.fixture.{}", std::process::id());
    let request = launch_request(Some(&id), None).expect("bundle request");
    let output = run_with_timeout(
        Command::new("swift")
            .arg(source)
            .arg(&first.path)
            .arg(&second.path),
        &serde_json::to_vec(&request).expect("JSON"),
        Duration::from_secs(35),
    )
    .expect("run helper");
    assert!(
        !output.status.success(),
        "bundle selector picked one installation: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("multiple application installations"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires an unlocked macOS desktop; launches a disposable application bundle"]
fn background_launch_discovers_app_and_does_not_reopen_existing_instance() {
    let fixture = Fixture::build();
    let launched = launch_app(None, Some(&fixture.path)).expect("background launch");
    assert_eq!(launched.status, LaunchStatus::Launched);
    assert!(!launched.foreground_changed);
    assert!(!launched.app.is_active);
    let state = fixture.state();
    assert_eq!(state["pid"], launched.app.pid);
    assert_eq!(state["activations"], 0);
    let app = list_apps()
        .expect("discover apps")
        .into_iter()
        .find(|app| app.pid == launched.app.pid)
        .expect("fixture is discoverable");
    assert!(app.finished_launching);
    let again = launch_app(None, Some(&fixture.path)).expect("existing instance");
    assert_eq!(again.status, LaunchStatus::AlreadyRunning);
    assert_eq!(again.app.pid, launched.app.pid);
    assert!(!again.foreground_changed);
    assert!(!again.app.is_active);
    thread::sleep(Duration::from_millis(200));
    assert_eq!(
        fixture.state(),
        state,
        "existing app must not receive reopen or activation"
    );
}

#[test]
#[ignore = "requires macOS application services and Swift Thread Sanitizer; launches a disposable app"]
fn launch_completion_has_no_cross_queue_data_race() {
    let fixture = Fixture::build();
    let source = fixture.directory.path().join("apps.swift");
    let binary = fixture.directory.path().join("apps-tsan");
    fs::write(&source, helper_source()).expect("production helper");
    let compiled = Command::new("swiftc")
        .args(["-sanitize=thread", "-g"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile sanitized helper");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let request = launch_request(None, Some(&fixture.path)).expect("launch request");
    let output = run_with_timeout(
        Command::new(binary).env("TSAN_OPTIONS", "halt_on_error=1:exitcode=66"),
        &serde_json::to_vec(&request).expect("request JSON"),
        Duration::from_secs(35),
    )
    .expect("run sanitized helper");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: LaunchResult =
        serde_json::from_slice(&output.stdout).expect("single launch result");
    assert_eq!(result.status, LaunchStatus::Launched);
    assert_eq!(fixture.state()["pid"], result.app.pid);
}
