use std::fs;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{InputDelivery, WindowScope, key, list_windows, scroll, snapshot_window, type_text};

pub(super) struct Receiver {
    child: Child,
    directory: tempfile::TempDir,
    windows: Vec<u32>,
}

impl Drop for Receiver {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Receiver {
    fn start(interrupt: bool) -> Self {
        Self::with_source(
            include_str!("input_fixture.swift"),
            if interrupt { "interrupt" } else { "normal" },
        )
    }

    pub(super) fn with_source(body: &str, mode: &str) -> Self {
        let directory = tempfile::tempdir().expect("fixture directory");
        let source = directory.path().join("receiver.swift");
        let binary = directory.path().join("Receiver");
        fs::write(&source, body).expect("fixture source");
        let compiled = Command::new("swiftc")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("swiftc");
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let child = Command::new(binary)
            .arg(directory.path().join("state.json"))
            .arg(mode)
            .spawn()
            .expect("receiver");
        let mut receiver = Self {
            child,
            directory,
            windows: Vec::new(),
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let mut windows: Vec<_> = list_windows(WindowScope::AllSpaces)
                .expect("catalog")
                .into_iter()
                .filter(|w| w.owner_pid == receiver.child.id() as i32)
                .collect();
            windows.sort_by(|a, b| a.title.cmp(&b.title));
            if windows.len() == 2 {
                receiver.windows = windows.into_iter().map(|w| w.window_id).collect();
                return receiver;
            }
            assert!(Instant::now() < deadline, "receiver did not appear");
            thread::sleep(Duration::from_millis(100));
        }
    }

    pub(super) fn state(&self) -> Value {
        serde_json::from_slice(
            &fs::read(self.directory.path().join("state.json")).expect("state file"),
        )
        .expect("state JSON")
    }

    pub(super) fn wait_for(&self, condition: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let state = self.state();
            if condition(&state) {
                return state;
            }
            assert!(
                Instant::now() < deadline,
                "unexpected receiver state: {state}"
            );
            thread::sleep(Duration::from_millis(30));
        }
    }

    pub(super) fn snapshot(&self, index: usize) -> super::WindowSnapshot {
        let path = self.directory.path().join(format!("window-{index}.png"));
        snapshot_window(self.windows[index], false, path.to_str()).expect("snapshot")
    }
}

#[test]
#[ignore = "requires a logged-in macOS desktop, Accessibility and Screen Recording permissions"]
fn background_input_routes_text_keys_and_scroll_without_activating_receiver() {
    let receiver = Receiver::start(false);
    let wrong = receiver.snapshot(0);
    let rejected = type_text(&wrong.input_target, "WRONG").expect_err("wrong keyboard window");
    assert!(
        rejected.to_string().contains("verified keyboard window"),
        "{rejected}"
    );
    assert_eq!(receiver.state()["texts"][1], "");
    let snapshot = receiver.snapshot(1);
    let text = "Unicode 測試 e\u{301} \u{1d538}";
    let sent = type_text(&snapshot.input_target, text).expect("text");
    assert_eq!(sent.status, InputDelivery::SentUnverified);
    assert_ne!(sent.frontmost_pid_before, receiver.child.id() as i32);
    assert_ne!(sent.frontmost_pid_after, receiver.child.id() as i32);
    receiver.wait_for(|s| s["texts"][1] == text);
    key(&snapshot.input_target, "enter", &[]).expect("Enter");
    type_text(&snapshot.input_target, "Next").expect("second line");
    receiver.wait_for(|s| s["texts"][1] == format!("{text}\nNext"));
    key(&snapshot.input_target, "a", &["command".into()]).expect("select all shortcut");
    type_text(&snapshot.input_target, "Replaced").expect("replace selection");
    receiver.wait_for(|s| s["texts"][1] == "Replaced");
    scroll(
        &wrong.input_target,
        f64::from(wrong.image.width) / 2.0,
        f64::from(wrong.image.height) / 2.0,
        0,
        -240,
    )
    .expect("scroll non-key window");
    let state = receiver.wait_for(|s| s["scroll_y"][0].as_f64().is_some_and(|y| y > 0.0));
    assert_eq!(state["active"], false);
    assert!(
        state["texts"][0]
            .as_str()
            .expect("text")
            .starts_with("Disposable line 1\n")
    );
}

#[test]
#[ignore = "requires a logged-in macOS desktop, Accessibility and Screen Recording permissions"]
fn background_input_stops_after_window_changes_and_releases_each_key() {
    let receiver = Receiver::start(true);
    let snapshot = receiver.snapshot(1);
    let result = type_text(&snapshot.input_target, &"a".repeat(500)).expect("partial result");
    assert_eq!(result.status, InputDelivery::PartiallySent);
    // AX and WindowServer publish the changed title independently.
    assert!(result.interruption.as_deref().is_some_and(|s|
        s.contains("window changed") || s.contains("verified keyboard window")), "{result:?}");
    assert!(result.events_sent >= 20 && result.events_sent < 1000);
    let state =
        receiver.wait_for(|s| s["events"].as_array().expect("events").len() == result.events_sent);
    let events = state["events"].as_array().expect("events");
    assert_eq!(state["titles"][1], "Changed input fixture");
    for pair in events.chunks_exact(2) {
        assert_eq!(pair[0]["type"], 10);
        assert_eq!(pair[1]["type"], 11);
        assert_eq!(pair[0]["flags"], 0);
        assert_eq!(pair[1]["flags"], 0);
    }
    assert_eq!(events.len() % 2, 0);
    assert_eq!(
        state["texts"][1].as_str().expect("text").len() * 2,
        result.events_sent
    );
}
