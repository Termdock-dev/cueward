use std::fs;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use super::inspect_window;
use crate::screenshot::list_capturable_windows;

struct FixtureProcess(Child);

impl Drop for FixtureProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "requires a logged-in macOS desktop and Accessibility permission"]
fn inspect_reads_a_real_window_button_and_value() {
    let directory = tempfile::tempdir().expect("temporary fixture directory");
    let source = directory.path().join("ax_fixture.swift");
    let executable = directory.path().join("CuewardAXFixture");
    let state = directory.path().join("state.txt");
    fs::write(&source, include_str!("ax_fixture.swift")).expect("write fixture");
    let compile = Command::new("swiftc")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("compile fixture");
    assert!(
        compile.status.success(),
        "fixture compilation failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let child = Command::new(&executable)
        .arg(&state)
        .spawn()
        .expect("launch fixture");
    let fixture = FixtureProcess(child);

    let deadline = Instant::now() + Duration::from_secs(10);
    let window_id = loop {
        let window = list_capturable_windows()
            .expect("list windows")
            .into_iter()
            .find(|window| {
                window.owner_pid == fixture.0.id() as i32 && window.title == "Cueward AX Fixture"
            });
        if let Some(window) = window {
            break window.window_id;
        }
        assert!(Instant::now() < deadline, "fixture window did not appear");
        thread::sleep(Duration::from_millis(200));
    };

    let result = inspect_window(window_id, 100, 6, false, false).expect("inspect fixture");
    assert_eq!(result.window.owner_pid, fixture.0.id() as i32);
    assert!(!result.accessibility.truncated);
    assert!(result.accessibility.nodes.iter().any(|node| {
        node.name == "Increment" && node.actions.iter().any(|action| action == "AXPress")
    }));
    assert!(
        result
            .accessibility
            .nodes
            .iter()
            .any(|node| node.value.as_deref() == Some("ready"))
    );
}
