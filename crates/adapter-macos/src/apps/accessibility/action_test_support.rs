use super::super::{AX_SUPPORT, AppAXActionResult, Target, now};
use crate::window::process::run_with_timeout;
use serde_json::{Value, json};
use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::process::{Child, Command, Output};
use std::time::Duration;

pub(super) struct Fixture {
    pub directory: tempfile::TempDir,
    service: Child,
    pub host: i32,
    pub receiver: i32,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.service.kill();
        let _ = self.service.wait();
    }
}

impl Fixture {
    pub fn build() -> Self {
        let directory = tempfile::tempdir().unwrap();
        for (name, body) in [
            ("inspect", include_str!("app_inspect.swift")),
            ("action", include_str!("app_action.swift")),
        ] {
            let source = format!(
                "{AX_SUPPORT}\n{}\n{}\n{}\n{body}",
                include_str!("action_fixture.swift"),
                include_str!("app_common.swift"),
                include_str!("app_roots.swift")
            );
            let path = directory.path().join(format!("{name}.swift"));
            fs::write(&path, source).unwrap();
            let compiled = run_with_timeout(
                Command::new("swiftc")
                    .arg(path)
                    .arg("-o")
                    .arg(directory.path().join(name)),
                b"",
                Duration::from_secs(40),
            )
            .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
        }
        let service = Command::new("/bin/sleep").arg("120").spawn().unwrap();
        let receiver = service.id() as i32;
        Self {
            directory,
            service,
            host: std::process::id() as i32,
            receiver,
        }
    }

    fn run(&self, operation: &str, scenario: &str, request: Value) -> Output {
        fs::write(self.directory.path().join("trace"), "").unwrap();
        run_with_timeout(
            Command::new(self.directory.path().join(operation))
                .env("APP_AX_SCENARIO", scenario)
                .env("APP_AX_HOST", self.host.to_string())
                .env("APP_AX_RECEIVER", self.receiver.to_string())
                .env("APP_AX_TRACE", self.directory.path().join("trace")),
            &serde_json::to_vec(&request).unwrap(),
            Duration::from_secs(10),
        )
        .unwrap()
    }

    pub fn observe(&self, scenario: &str, reference: &str) -> Target {
        let root = reference.split('.').next().unwrap();
        let output = self.run(
            "inspect",
            scenario,
            json!({"pid": self.host, "root": root, "limit": 100, "depth": 3}),
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: Value = serde_json::from_slice(&output.stdout).unwrap();
        let node = snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["ref"] == reference)
            .unwrap();
        serde_json::from_value(
            json!({"kind": "app_ax", "version": 1, "issued_at": now().unwrap(),
            "app": snapshot["app"], "ref": reference, "root_fingerprint": node["root_fingerprint"],
            "context": snapshot["context"], "fingerprint": node["fingerprint"]}),
        )
        .unwrap()
    }

    pub fn act(&self, scenario: &str, target: &Target, action: &str) -> Output {
        self.run("action", scenario, json!({"pid": self.host, "target": target, "action": action,
            "value": "Fixture text\n繁中", "caller_pid": std::process::id(), "lock_dir": self.directory.path()}))
    }

    pub fn events(&self) -> Vec<String> {
        fs::read_to_string(self.directory.path().join("trace"))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    pub fn reject(&self, scenario: &str, target: &Target, action: &str, message: &str) {
        let output = self.act(scenario, target, action);
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{scenario} accepted {action}");
        assert!(error.contains(message), "{scenario}: {error}");
        assert!(
            !self.events().iter().any(|e| e.starts_with("dispatch:")),
            "{scenario} dispatched input"
        );
    }

    pub fn accept(&self, scenario: &str, target: &Target, action: &str) -> AppAXActionResult {
        let output = self.act(scenario, target, action);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let events = self.events();
        assert_eq!(
            events
                .iter()
                .filter(|e| e.starts_with("dispatch:"))
                .collect::<Vec<_>>(),
            [&format!("dispatch:{action}")]
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    pub fn hold_lock(&self, pid: i32) -> File {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.directory.path().join(format!("input-{pid}.lock")))
            .unwrap();
        assert_eq!(unsafe { flock(file.as_raw_fd(), 0x02 | 0x04) }, 0);
        file
    }
}

unsafe extern "C" {
    fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
}
