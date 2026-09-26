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

struct Fixture {
    directory: tempfile::TempDir,
    path: std::path::PathBuf,
}

impl Fixture {
    fn build() -> Self {
        let directory = tempfile::tempdir().expect("app fixture directory");
        let path = directory.path().join("CuewardApplicationFixture.app");
        let contents = path.join("Contents");
        fs::create_dir_all(contents.join("MacOS")).expect("bundle directories");
        let source = directory.path().join("fixture.swift");
        fs::write(&source, include_str!("fixture.swift")).expect("fixture source");
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
        if let Ok(data) = fs::read(self.directory.path().join("state.json")) {
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
