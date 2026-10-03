//! Shared owned-process setup for interruption tests; operation effects stay in each suite.
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub(super) fn wait_for(mut predicate: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !predicate() {
        assert!(
            std::time::Instant::now() < deadline,
            "owned helper timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

pub(super) fn config(env: &str) -> Option<(PathBuf, serde_json::Value)> {
    let path = PathBuf::from(std::env::var_os(env)?);
    let value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    Some((path.parent().unwrap().into(), value))
}

pub(super) struct Processes {
    parent: Option<Child>,
    group: Option<String>,
}
impl Drop for Processes {
    fn drop(&mut self) {
        if let Some(parent) = &mut self.parent {
            let _ = parent.kill();
            let _ = parent.wait();
        }
        if let Some(group) = &self.group {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", &format!("-{group}")])
                .status();
        }
    }
}

pub(super) fn launch(
    control: &Path,
    mut config: serde_json::Value,
    env: &str,
    parent_helper: &str,
    worker_helper: &str,
) -> Processes {
    let exe = std::env::current_exe().unwrap();
    let worker = control.join("worker");
    fs::write(&worker, format!(
        "#!/bin/sh\nprintf '%s' \"$$\" > '{0}/group'\n'{1}' --exact {worker_helper} --nocapture\nprintf '%s' \"$?\" > '{0}/finished'\n",
        control.display(), exe.display()
    )).unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    config["worker"] = serde_json::json!(worker);
    let path = control.join("config.json");
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let parent = Command::new(exe)
        .args(["--exact", parent_helper, "--nocapture"])
        .env(env, path)
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(control.join("progress")).unwrap()))
        .spawn()
        .unwrap();
    Processes {
        parent: Some(parent),
        group: None,
    }
}

impl Processes {
    /// Stop at a known checkpoint and return the ID announced before dispatch.
    pub(super) fn interrupt(&mut self, control: &Path, signal: Option<&str>) -> String {
        wait_for(|| control.join("ready").exists());
        self.group = Some(fs::read_to_string(control.join("group")).unwrap());
        let parent = self.parent.as_mut().unwrap();
        if let Some(signal) = signal {
            assert!(
                Command::new("/bin/kill")
                    .args([signal, &parent.id().to_string()])
                    .status()
                    .unwrap()
                    .success()
            );
            assert!(!parent.wait().unwrap().success());
            fs::write(control.join("release"), b"").unwrap();
            wait_for(|| fs::metadata(control.join("finished")).is_ok_and(|m| m.len() > 0));
            assert_ne!(fs::read_to_string(control.join("finished")).unwrap(), "0");
        } else {
            // A surviving parent records the supervisor's conservative timeout result.
            assert!(parent.wait().unwrap().success());
        }
        self.parent = None;
        self.group = None;
        fs::read_to_string(control.join("progress"))
            .unwrap()
            .split("operation_id=")
            .nth(1)
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned()
    }
}
