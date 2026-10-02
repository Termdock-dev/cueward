//! Parent death and deadlines stop the real worker even after private payload progress.
use super::*;
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Stdio};
const ENV: &str = "CUEWARD_TEST_TREE_LIFETIME_CONFIG";
const PREFIX: &str = "files::copy_tree::execution::safety_tests::lifetime::";

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
fn tree_parent_helper() {
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
fn tree_worker_helper() {
    let Some((control, value)) = config() else {
        return;
    };
    let input = read_supervised_request().unwrap();
    if value["mode"] == "armed" {
        fs::write(control.join("ready"), b"").unwrap();
        wait_for(|| control.join("release").exists());
        execute_worker(&input).unwrap();
    } else {
        let mut platform = racing();
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
fn launch(control: &Path, request: &CopyTreeRequest, mode: &str, timeout: u64) -> Processes {
    let exe = std::env::current_exe().unwrap();
    let worker = control.join("worker");
    fs::write(&worker, format!("#!/bin/sh\nprintf '%s' \"$$\" > '{0}/group'\n'{1}' --exact {PREFIX}tree_worker_helper --nocapture\nprintf '%s' \"$?\" > '{0}/finished'\n",control.display(),exe.display())).unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    let config = control.join("config.json");
    fs::write(
        &config,
        serde_json::to_vec(
            &serde_json::json!({"worker":worker,"request":request,"mode":mode,"timeout":timeout}),
        )
        .unwrap(),
    )
    .unwrap();
    let parent = Command::new(exe)
        .args([
            "--exact",
            &format!("{PREFIX}tree_parent_helper"),
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
    let root = fixture();
    let control = tempfile::tempdir().unwrap();
    let source_version = request(root.path()).expected_version;
    let mut processes = launch(
        control.path(),
        &request(root.path()),
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
    assert!(!receipt.completion_verified && !receipt.mutation_attempted);
    assert_eq!(receipt.destination_created, None);
    assert!(!root.path().join("destination").exists());
    assert_eq!(request(root.path()).expected_version, source_version);
    assert_eq!(
        fs::read(root.path().join("source/.hidden")).unwrap(),
        b"OWNED hidden\0"
    );
    if mode == "partial" {
        assert_eq!(receipt.stage, TreeStage::Copying);
        let staged = Path::new(receipt.staging_path.as_ref().unwrap());
        assert_eq!(fs::read(staged.join(".hidden")).unwrap(), b"OWNED hidden\0");
        assert!(!receipt.staging_verified);
    } else {
        assert_eq!(receipt.stage, TreeStage::Preflight);
        assert!(receipt.staging_path.is_none());
    }
    let directory = Path::new(&receipt.receipt_path).parent().unwrap();
    assert_eq!(directory.file_name().unwrap().to_str().unwrap(), id);
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn tree_execute_parent_signals_stop_armed_and_partially_copied_workers() {
    for mode in ["armed", "partial"] {
        for signal in ["-INT", "-TERM", "-KILL"] {
            interrupted(mode, Some(signal));
        }
    }
}
#[test]
fn tree_execute_deadline_after_private_copy_preserves_uncertain_receipt() {
    interrupted("partial", None);
}
