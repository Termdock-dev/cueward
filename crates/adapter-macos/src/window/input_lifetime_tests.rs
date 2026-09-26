use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::input_live_tests::Receiver;
use super::input_lock::lock_input;
use super::type_text;

struct InputChild {
    child: Child,
    helpers: Vec<u32>,
    group_anchor: Option<Child>,
}

impl InputChild {
    fn helper_signal(&self, signal: &str) {
        let ids: Vec<_> = self.helpers.iter().map(u32::to_string).collect();
        let result = Command::new("/bin/kill")
            .args([signal, "--"])
            .args(&ids)
            .status()
            .expect("signal owned helpers");
        assert!(result.success());
    }

    fn find_helper(&mut self) {
        let output = Command::new("/bin/ps")
            .args(["-axo", "pid=,ppid="])
            .output()
            .expect("process inventory");
        assert!(output.status.success());
        let rows: Vec<(u32, u32)> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let ids: Vec<u32> = line
                    .split_whitespace()
                    .filter_map(|v| v.parse().ok())
                    .collect();
                (ids.len() == 2).then(|| (ids[0], ids[1]))
            })
            .collect();
        let mut parents = vec![self.child.id()];
        while let Some(parent) = parents.pop() {
            for &(pid, owner) in &rows {
                if owner == parent {
                    self.helpers.push(pid);
                    parents.push(pid);
                }
            }
        }
        assert!(!self.helpers.is_empty(), "owned helper processes missing");
    }
}

impl Drop for InputChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(anchor) = self.group_anchor.as_mut() {
            let _ = anchor.kill();
            let _ = anchor.wait();
        }
        if !self.helpers.is_empty() {
            let ids: Vec<_> = self.helpers.iter().map(u32::to_string).collect();
            let _ = Command::new("/bin/kill")
                .args(["-KILL", "--"])
                .args(&ids)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

#[test]
#[ignore = "requires a logged-in macOS desktop, Accessibility and Screen Recording permissions"]
fn input_helper_retains_lock_and_stops_when_caller_exits() {
    const TOKEN_FILE: &str = "CUEWARD_TEST_INPUT_TOKEN_FILE";
    if let Some(path) = std::env::var_os(TOKEN_FILE) {
        let token = std::fs::read_to_string(path).expect("test input token");
        type_text(&token, &"a".repeat(1000)).expect("child text");
        return;
    }
    let receiver = Receiver::start(false);
    let snapshot = receiver.snapshot(1);
    let directory = tempfile::tempdir().expect("test files");
    let path = directory.path().join("token");
    std::fs::write(&path, &snapshot.input_target).expect("token file");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "window::input_lifetime_tests::input_helper_retains_lock_and_stops_when_caller_exits",
            "--ignored",
        ])
        .env(TOKEN_FILE, &path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("input caller");
    let mut input = InputChild {
        child,
        helpers: Vec::new(),
        group_anchor: None,
    };
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        if receiver.state()["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|e| e["tag"] == 0x43554549_u64 && e["type"] == 10)
        {
            break;
        }
        assert!(
            input.child.try_wait().expect("caller status").is_none(),
            "input caller exited before key-down"
        );
        assert!(Instant::now() < deadline, "no key-down");
        thread::sleep(Duration::from_millis(10));
    }
    input.find_helper();
    // Keep the group non-orphaned while stopped: macOS otherwise sends SIGHUP
    // when its parent dies, which would test OS job control instead of input lifetime.
    input.group_anchor = Some(
        Command::new("/bin/sleep")
            .arg("15")
            .process_group(input.helpers[0] as i32)
            .spawn()
            .expect("owned group anchor"),
    );
    input.helper_signal("-STOP");
    input.child.kill().expect("stop caller");
    input.child.wait().expect("reap caller");
    thread::sleep(Duration::from_millis(50));
    let count = receiver.state()["events"].as_array().expect("events").len();
    let overlap = lock_input(snapshot.window.owner_pid);
    assert!(
        overlap.is_err(),
        "input lock was released while a stopped helper could still send input"
    );
    input.helper_signal("-CONT");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if lock_input(snapshot.window.owner_pid).is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "orphaned input helper did not stop"
        );
        thread::sleep(Duration::from_millis(20));
    }
    thread::sleep(Duration::from_millis(50));
    let state = receiver.state();
    let events = state["events"].as_array().expect("events");
    assert!(
        events.len() <= count + 2,
        "orphaned helper continued typing after caller exit"
    );
    assert_eq!(events.len() % 2, 0);
    for pair in events.chunks_exact(2) {
        assert_eq!(pair[0]["type"], 10);
        assert_eq!(pair[1]["type"], 11);
    }
}
