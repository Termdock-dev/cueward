use super::*;
use serde_json::Value;
use std::fs;
use std::os::fd::AsRawFd;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

struct Fixture {
    child: Child,
    directory: tempfile::TempDir,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Fixture {
    fn launch() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("fixture.swift");
        let binary = directory.path().join("Fixture");
        fs::write(&source, include_str!("live_fixture.swift")).unwrap();
        let output = Command::new("swiftc")
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
        let child = Command::new(binary)
            .arg(directory.path().join("state.json"))
            .spawn()
            .unwrap();
        let fixture = Self { child, directory };
        fixture.wait_state("window", json!(false));
        fixture
    }
    fn pid(&self) -> i32 {
        self.child.id() as i32
    }
    fn state(&self) -> Option<Value> {
        fs::read(self.directory.path().join("state.json"))
            .ok()
            .and_then(|v| serde_json::from_slice(&v).ok())
    }
    fn wait_state(&self, key: &str, expected: Value) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.state().is_some_and(|s| s[key] == expected) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "state mismatch: {:?}",
                self.state()
            );
            thread::sleep(Duration::from_millis(50));
        }
    }
    fn inspect(&self, root: Option<&str>) -> AppAccessibilitySnapshot {
        inspect_app(self.pid(), root, 300, 12).unwrap_or_else(|e| panic!("inspect {root:?}: {e}"))
    }
}

fn named_target(snapshot: &AppAccessibilitySnapshot, name: &str) -> String {
    snapshot
        .nodes
        .iter()
        .find(|item| item.node.name == name)
        .and_then(|item| item.node.target.clone())
        .unwrap_or_else(|| panic!("missing fixture target: {name}"))
}

unsafe extern "C" {
    fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
}

fn assert_input_lock_blocks(pid: i32, token: &str) {
    let path = std::path::PathBuf::from(crate::screenshot::ensure_cache_dir().unwrap())
        .join(format!("input-{pid}.lock"));
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .unwrap();
    assert_eq!(unsafe { flock(lock.as_raw_fd(), 0x02 | 0x04) }, 0);
    let error = press_app_element(token).unwrap_err().to_string();
    assert!(error.contains("another input action is running"), "{error}");
}

fn open_window(fixture: &Fixture) {
    let roots = fixture.inspect(None);
    assert!(roots.nodes.iter().all(|item| item.node.r#ref == "menu"));
    assert!(!roots.nodes.is_empty());
    let menu = fixture.inspect(Some("menu"));
    let disabled = menu
        .nodes
        .iter()
        .find(|item| item.node.name == "Disabled Fixture")
        .unwrap();
    assert!(disabled.node.target.is_none());
    let result = press_app_element(&named_target(&menu, "New Fixture")).unwrap();
    assert_eq!(result.status, ActionStatus::SentUnverified);
    assert!(!result.foreground_changed);
    fixture.wait_state("window", json!(true));
    assert!(
        press_app_element(&named_target(&menu, "New Fixture")).is_err(),
        "old menu context accepted"
    );
}

fn edit_window(fixture: &Fixture) {
    let window = fixture.inspect(Some("w0"));
    let secret = window
        .nodes
        .iter()
        .find(|item| item.node.subrole.as_deref() == Some("AXSecureTextField"))
        .unwrap();
    assert!(secret.node.value.is_none() && secret.node.target.is_none());
    let field = window
        .nodes
        .iter()
        .find(|item| item.node.identifier.as_deref() == Some("fixture-text"))
        .unwrap();
    let token = field.node.target.as_deref().unwrap();
    let mut wrong_instance = Target::decode(token, now().unwrap()).unwrap();
    wrong_instance.app.start_seconds += 1;
    let error = set_app_value(&wrong_instance.encode().unwrap(), "wrong instance").unwrap_err();
    assert!(error.to_string().contains("application instance changed"));
    assert_input_lock_blocks(fixture.pid(), token);
    let subtree = fixture.inspect(Some(&field.node.r#ref));
    assert_eq!(subtree.nodes[0].node.fingerprint, field.node.fingerprint);
    let result = set_app_value(token, "Updated fixture").unwrap();
    assert_eq!(result.status, ActionStatus::Confirmed);
    assert!(!result.foreground_changed);
    fixture.wait_state("text", json!("Updated fixture"));
    assert!(set_app_value(token, "stale overwrite").is_err());
    fixture.wait_state("text", json!("Updated fixture"));
}

fn cancel_panel(fixture: &Fixture) {
    let window = fixture.inspect(Some("w0"));
    let result = press_app_element(&named_target(&window, "Open Fixture Panel")).unwrap();
    assert!(!result.foreground_changed);
    let roots = fixture.inspect(None);
    let mut target = None;
    for item in roots
        .nodes
        .iter()
        .filter(|item| item.node.r#ref.starts_with('w'))
    {
        let snapshot = fixture.inspect(Some(&item.node.r#ref));
        // Service ownership varies by OS. When present, exercise the receiver's
        // actual input lock without changing shared file-panel preferences.
        if let Some(remote) = snapshot
            .nodes
            .iter()
            .find(|n| n.receiver_pid != fixture.pid() && n.node.target.is_some())
        {
            assert_input_lock_blocks(remote.receiver_pid, remote.node.target.as_deref().unwrap());
        }
        if let Some(cancel) = snapshot
            .nodes
            .iter()
            .find(|n| n.node.name == "Cancel" && n.node.role == "AXButton")
        {
            target = cancel.node.target.clone();
            break;
        }
    }
    let result = press_app_element(&target.expect("discover panel Cancel button")).unwrap();
    assert!(!result.foreground_changed);
    fixture.wait_state("result", json!("cancelled"));
}

#[test]
#[ignore = "requires an unlocked desktop and Accessibility; opens only a disposable app and panel"]
fn app_ax_explores_windowless_menu_text_and_system_panel() {
    let fixture = Fixture::launch();
    open_window(&fixture);
    edit_window(&fixture);
    cancel_panel(&fixture);
    fixture.wait_state("activations", json!(0));
    fixture.wait_state("active", json!(false));
}
