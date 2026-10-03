//! Parent death and deadlines stop the real worker even after private payload progress.
use super::tests::*;
use super::*;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
const ENV: &str = "CUEWARD_TEST_TRASH_LIFETIME_CONFIG";
const PREFIX: &str = "files::trash::execution::lifetime_tests::";

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
fn config() -> Option<(PathBuf, serde_json::Value)> {
    let path = PathBuf::from(std::env::var_os(ENV)?);
    let value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    Some((path.parent().unwrap().into(), value))
}
#[test]
fn trash_parent_helper() {
    let Some((_, value)) = config() else { return };
    let request = serde_json::from_value(value["request"].clone()).unwrap();
    let receipt = run(
        Path::new(value["worker"].as_str().unwrap()),
        &request,
        value["timeout"].as_u64().unwrap(),
    )
    .unwrap();
    assert_eq!(receipt.status, MutationStatus::Uncertain);
}
#[test]
fn trash_worker_helper() {
    let Some((control, value)) = config() else {
        return;
    };
    let input = read_supervised_request().unwrap();
    if value["mode"] == "armed" {
        fs::write(control.join("ready"), b"").unwrap();
        wait_for(|| control.join("release").exists());
        execute_worker(&input).unwrap();
    } else {
        let mut platform = Controlled {
            trash: PathBuf::from(value["trash"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            race: None,
            uncertain: false,
            pause: None,
            unknown_alias: false,
            unknown_data: false,
            native_mode: 0,
            storage_inside: false,
            native_pause: match value["mode"].as_str().unwrap() {
                "native" => 1,
                "trashed" => 2,
                _ => 0,
            },
        };
        platform.pause = Some(control);
        execute_prepared(&platform, &input).unwrap();
    }
}
struct Processes {
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
fn launch(
    control: &Path,
    request: &TrashRequest,
    trash: &Path,
    mode: &str,
    timeout: u64,
) -> Processes {
    let exe = std::env::current_exe().unwrap();
    let worker = control.join("worker");
    fs::write(&worker, format!("#!/bin/sh\nprintf '%s' \"$$\" > '{0}/group'\n'{1}' --exact {PREFIX}trash_worker_helper --nocapture\nprintf '%s' \"$?\" > '{0}/finished'\n",control.display(),exe.display())).unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    let config = control.join("config.json");
    fs::write(
        &config,
        serde_json::to_vec(
            &serde_json::json!({"worker":worker,"request":request,"mode":mode,"timeout":timeout,"trash":trash}),
        )
        .unwrap(),
    )
    .unwrap();
    let parent = Command::new(exe)
        .args([
            "--exact",
            &format!("{PREFIX}trash_parent_helper"),
            "--nocapture",
        ])
        .env(ENV, config)
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(control.join("progress")).unwrap()))
        .spawn()
        .unwrap();
    Processes {
        parent: Some(parent),
        group: None,
    }
}
fn interrupted(mode: &str, signal: Option<&str>) {
    let value = fixture();
    let root = &value.root;
    let control = tempfile::tempdir().unwrap();
    let source_version = request(root.path()).expected_version;
    let mut processes = launch(
        control.path(),
        &request(root.path()),
        value.trash.path(),
        mode,
        if signal.is_some() { 30000 } else { 2000 },
    );
    wait_for(|| control.path().join("ready").exists());
    processes.group = Some(fs::read_to_string(control.path().join("group")).unwrap());
    let parent = processes.parent.as_mut().unwrap();
    if let Some(signal) = signal {
        assert!(
            Command::new("/bin/kill")
                .args([signal, &parent.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        assert!(!parent.wait().unwrap().success());
        fs::write(control.path().join("release"), b"").unwrap();
        wait_for(|| fs::metadata(control.path().join("finished")).is_ok_and(|m| m.len() > 0));
        assert_ne!(
            fs::read_to_string(control.path().join("finished")).unwrap(),
            "0"
        );
    } else {
        // Parent survives the deadline and saves conservative transport evidence.
        assert!(parent.wait().unwrap().success());
    }
    processes.parent = None;
    processes.group = None;
    let progress = fs::read_to_string(control.path().join("progress")).unwrap();
    let id = progress
        .split("operation_id=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let receipt = read_receipt(id).unwrap();
    assert_eq!(receipt.status, MutationStatus::Uncertain);
    assert!(!receipt.completion_verified);
    assert_eq!(receipt.moved_to_trash, None);
    if mode == "native" || mode == "trashed" {
        assert!(receipt.mutation_attempted && receipt.backup_verified && receipt.trash_attempted);
        assert_eq!(receipt.source_removed, Some(true));
        assert_eq!(receipt.stage, TrashStage::Trashing);
        assert!(!root.path().join("source").exists());
        let moved = if mode == "native" {
            PathBuf::from(receipt.staging_path.as_ref().unwrap())
        } else {
            value.trash.path().join("source")
        };
        assert_eq!(fs::read(moved).unwrap(), b"OWNED bytes\0</external>");
        assert_eq!(
            fs::read(receipt.backup_path.as_ref().unwrap()).unwrap(),
            b"OWNED bytes\0</external>"
        );
    } else {
        assert!(!receipt.mutation_attempted);
        assert_eq!(receipt.source_removed, None);
        assert_eq!(fs::read_dir(value.trash.path()).unwrap().count(), 0);
        assert_eq!(request(root.path()).expected_version, source_version);
        assert_eq!(
            fs::read(root.path().join("source")).unwrap(),
            b"OWNED bytes\0</external>"
        );
        if mode == "partial" {
            assert_eq!(receipt.stage, TrashStage::BackingUp);
            assert_eq!(
                fs::read(receipt.backup_path.as_ref().unwrap()).unwrap(),
                b"OWNED bytes\0</external>"
            );
            assert!(!receipt.backup_verified);
        } else {
            assert_eq!(receipt.stage, TrashStage::Preflight);
            assert!(receipt.backup_path.is_none());
        }
    }
    let directory = Path::new(&receipt.receipt_path).parent().unwrap();
    assert_eq!(directory.file_name().unwrap().to_str().unwrap(), id);
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn trash_execute_parent_signals_stop_armed_and_partially_copied_workers() {
    for mode in ["armed", "partial", "native", "trashed"] {
        for signal in ["-INT", "-TERM", "-KILL"] {
            interrupted(mode, Some(signal));
        }
    }
}
#[test]
fn trash_execute_deadline_after_private_copy_preserves_uncertain_receipt() {
    for mode in ["partial", "native", "trashed"] {
        interrupted(mode, None);
    }
}
