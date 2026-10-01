//! Parent interruption must prevent a delayed writer from creating its destination.
use super::*;
use cueward_adapter_macos::files::mutation::{self, MutationAction, MutationRequest};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

#[test]
fn cancellation_parent_helper() {
    let Some(config) = std::env::var_os("CUEWARD_TEST_CANCELLATION_CONFIG") else {
        return;
    };
    let value: Value = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    let request: MutationRequest = serde_json::from_value(value["request"].clone()).unwrap();
    let _ = mutation::run(
        Path::new(value["worker"].as_str().unwrap()),
        &request,
        30000,
    );
}

struct Processes {
    parent: Option<std::process::Child>,
    worker: Option<String>,
}
impl Drop for Processes {
    fn drop(&mut self) {
        if let Some(child) = self.parent.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(pid) = &self.worker {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{pid}")])
                .output();
        }
    }
}
fn wait_for(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !predicate() {
        assert!(Instant::now() < end, "timed out waiting for test barrier");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn prepare(root: &Path, action: &str) -> MutationRequest {
    fs::write(root.join("source"), b"owned source bytes").unwrap();
    let parent = super::mutation::version(root, ".");
    let action = if action == "mkdir" {
        MutationAction::Mkdir
    } else {
        MutationAction::Copy {
            path: "source".into(),
            expected_version: super::mutation::version(root, "source"),
            max_bytes: 1024,
        }
    };
    MutationRequest {
        root: root.into(),
        destination: "destination".into(),
        expected_parent_version: parent,
        action,
    }
}
fn launch(directory: &Path, request: &MutationRequest) -> Processes {
    let wrapper = directory.join("worker");
    // Delay exec, leaving the actual worker's stdin untouched, including its lifeline.
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > '{0}/ready'\nwhile [ ! -e '{0}/release' ]; do /bin/sleep 0.01; done\nexec '{1}' files-mutation-worker\n",
            directory.display(), env!("CARGO_BIN_EXE_cueward")
        ),
    ).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let config = directory.join("config.json");
    fs::write(
        &config,
        serde_json::to_vec(&json!({"request":request,"worker":wrapper})).unwrap(),
    )
    .unwrap();
    let parent = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cancellation::cancellation_parent_helper",
            "--nocapture",
        ])
        .env("CUEWARD_TEST_CANCELLATION_CONFIG", config)
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            fs::File::create(directory.join("progress")).unwrap(),
        ))
        .spawn()
        .unwrap();
    Processes {
        parent: Some(parent),
        worker: None,
    }
}
fn receipt_for(root: &Path) -> Value {
    let operations = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap()
        .join(".cueward/operations/files");
    let receipts: Vec<Value> = fs::read_dir(operations)
        .unwrap()
        .filter_map(|entry| {
            let bytes = fs::read(entry.ok()?.path().join("receipt.json")).ok()?;
            let value: Value = serde_json::from_slice(&bytes).ok()?;
            (value["request"]["root"].as_str() == root.to_str()).then_some(value)
        })
        .collect();
    assert_eq!(receipts.len(), 1, "one owned operation receipt expected");
    receipts.into_iter().next().unwrap()
}
fn interrupt_delayed(action: &str, signal: &str) {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("root");
    fs::create_dir(&root).unwrap();
    let request = prepare(&root, action);
    let mut processes = launch(fixture.path(), &request);
    wait_for(|| fixture.path().join("ready").exists());
    processes.worker = Some(fs::read_to_string(fixture.path().join("ready")).unwrap());
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
    fs::write(fixture.path().join("release"), b"").unwrap();
    // Allow the delayed worker to run and prove that parent death prevents its mutation.
    std::thread::sleep(Duration::from_millis(500));
    let receipt = receipt_for(&root);
    let created = root.join("destination").exists();
    super::mutation::cleanup(&receipt);
    assert!(!created, "{action} worker wrote after parent {signal}");
    assert_eq!(receipt["status"], "uncertain");
    assert_eq!(receipt["completion_verified"], false);
    assert_eq!(
        fs::read(root.join("source")).unwrap(),
        b"owned source bytes"
    );
    let progress = fs::read_to_string(fixture.path().join("progress")).unwrap();
    assert!(
        progress.contains(receipt["operation_id"].as_str().unwrap()),
        "operation ID must be published before dispatch"
    );
}
#[test]
fn parent_interruption_prevents_delayed_mkdir_and_copy() {
    for action in ["mkdir", "copy"] {
        for signal in ["-INT", "-TERM", "-KILL"] {
            interrupt_delayed(action, signal);
        }
    }
}
