//! The same lifeline must stop a batch after native progress, not just before dispatch.
use super::*;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

pub(super) fn wait_for(mut predicate: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !predicate() {
        assert!(
            std::time::Instant::now() < deadline,
            "owned helper did not reach expected checkpoint"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
fn config() -> Option<(std::path::PathBuf, serde_json::Value)> {
    let path = std::path::PathBuf::from(std::env::var_os("CUEWARD_TEST_BATCH_PARTIAL_CONFIG")?);
    let value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    Some((path, value))
}
#[test]
fn partial_parent_helper() {
    let Some((_, value)) = config() else {
        return;
    };
    let request: BatchRenameRequest = serde_json::from_value(value["request"].clone()).unwrap();
    let _ = run(
        Path::new(value["worker"].as_str().unwrap()),
        &request,
        30000,
    );
}
#[test]
fn partial_worker_helper() {
    let Some((path, value)) = config() else {
        return;
    };
    let input = read_supervised_request().unwrap();
    let platform = Racing {
        root: value["request"]["root"].as_str().unwrap().into(),
        calls: Cell::new(0),
        change: Change::Pause(path.parent().unwrap().into()),
    };
    execute_prepared(&platform, &input).unwrap();
}
struct Processes {
    parent: Option<std::process::Child>,
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
fn launch(directory: &Path, request: &BatchRenameRequest) -> Processes {
    let worker = directory.join("worker");
    let exe = std::env::current_exe().unwrap();
    fs::write(&worker,format!("#!/bin/sh\nprintf '%s' \"$$\" > '{0}/group'\n'{1}' --exact files::batch_execution::tests::races::lifetime::partial_worker_helper --nocapture\nprintf '%s' \"$?\" > '{0}/finished'\n",directory.display(),exe.display())).unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    let config = directory.join("config.json");
    fs::write(
        &config,
        serde_json::to_vec(&serde_json::json!({"request":request,"worker":worker})).unwrap(),
    )
    .unwrap();
    let parent = Command::new(exe)
        .args([
            "--exact",
            "files::batch_execution::tests::races::lifetime::partial_parent_helper",
            "--nocapture",
        ])
        .env("CUEWARD_TEST_BATCH_PARTIAL_CONFIG", config)
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            File::create(directory.join("progress")).unwrap(),
        ))
        .spawn()
        .unwrap();
    Processes {
        parent: Some(parent),
        group: None,
    }
}
fn interrupt(signal: &str) {
    let root = fixture();
    let transport = tempfile::tempdir().unwrap();
    let request = request(root.path(), &[("a", "new"), ("b", "other")]);
    let mut processes = launch(transport.path(), &request);
    wait_for(|| transport.path().join("ready").exists());
    processes.group = Some(fs::read_to_string(transport.path().join("group")).unwrap());
    let parent = processes.parent.as_mut().unwrap();
    assert!(
        Command::new("/bin/kill")
            .args([signal, &parent.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    assert!(!parent.wait().unwrap().success());
    processes.parent = None;
    fs::write(transport.path().join("release"), b"").unwrap();
    wait_for(|| fs::metadata(transport.path().join("finished")).is_ok_and(|m| m.len() > 0));
    processes.group = None;
    assert_ne!(
        fs::read_to_string(transport.path().join("finished")).unwrap(),
        "0"
    );
    let progress = fs::read_to_string(transport.path().join("progress")).unwrap();
    let id = progress
        .split("operation_id=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let receipt = read_receipt(id).unwrap();
    assert_eq!(receipt.status, RelocationStatus::Uncertain);
    assert!(!receipt.completion_verified && receipt.items[0].mutation_attempted);
    assert_eq!(receipt.items[0].renamed, None);
    assert!(receipt.items[1].operation_id.is_none());
    assert_eq!(fs::read(root.path().join("new")).unwrap(), b"OWNED a\0");
    assert_eq!(fs::read(root.path().join("b")).unwrap(), b"OWNED b\0");
    assert!(!root.path().join("a").exists() && !root.path().join("other").exists());
    cleanup(&receipt);
}
#[test]
fn batch_execute_parent_death_after_first_native_rename_preserves_partial_evidence() {
    for signal in ["-INT", "-TERM", "-KILL"] {
        interrupt(signal);
    }
}
