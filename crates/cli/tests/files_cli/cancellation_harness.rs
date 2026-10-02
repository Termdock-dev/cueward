//! Shared delayed/armed-parent interruption harness using only owned fixtures.
use super::cancellation::{Processes, wait_for};
use super::*;
use std::os::unix::fs::PermissionsExt;
fn nonempty(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0)
}
fn worker_script(directory: &Path, armed: bool, module: &str, worker: &str) -> String {
    let body = if armed {
        format!(
            "'{}' --exact {module}::cancellation_worker_helper --nocapture",
            std::env::current_exe().unwrap().display()
        )
    } else {
        format!(
            "printf '%s' \"$$\" > '{0}/ready'\nwhile [ ! -e '{0}/release' ]; do /bin/sleep 0.01; done\n'{1}' {worker}",
            directory.display(),
            env!("CARGO_BIN_EXE_cueward")
        )
    };
    format!(
        "#!/bin/sh\nprintf '%s' \"$$\" > '{}/group'\n{body}\nprintf '%s' \"$?\" > '{}/finished'\n",
        directory.display(),
        directory.display()
    )
}
fn launch(
    directory: &Path,
    request: &impl serde::Serialize,
    armed: bool,
    module: &str,
    worker: &str,
) -> Processes {
    let wrapper = directory.join("worker");
    // Keep stdin intact; the wrapper reports completion so an unscheduled worker cannot pass.
    fs::write(&wrapper, worker_script(directory, armed, module, worker)).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let config = directory.join("config.json");
    fs::write(
        &config,
        serde_json::to_vec(&json!({"request":request,"worker":wrapper})).unwrap(),
    )
    .unwrap();
    let helper = format!("{module}::cancellation_parent_helper");
    let parent = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &helper, "--nocapture"])
        .env("CUEWARD_TEST_CANCEL_CONFIG", config)
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
fn receipt_for(root: &Path, namespace: &str) -> Value {
    let operations = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap()
        .join(".cueward/operations")
        .join(namespace);
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
pub(super) struct Harness {
    pub module: &'static str,
    pub worker: &'static str,
    pub namespace: &'static str,
    pub lookup: &'static str,
}
impl Harness {
    pub(super) fn interrupt(
        &self,
        directory: &Path,
        root: &Path,
        request: &impl serde::Serialize,
        signal: &str,
        armed: bool,
    ) {
        let Self {
            module,
            worker,
            namespace,
            lookup,
        } = self;
        let mut processes = launch(directory, request, armed, module, worker);
        wait_for(|| nonempty(&directory.join("ready")));
        processes.worker = Some(fs::read_to_string(directory.join("group")).unwrap());
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
        fs::write(directory.join("release"), b"").unwrap();
        wait_for(|| nonempty(&directory.join("finished")));
        processes.worker = None; // The wrapper has reaped the worker; do not signal a recycled PID.
        assert_ne!(fs::read_to_string(directory.join("finished")).unwrap(), "0");
        verify_interrupted(root, directory, namespace, lookup, signal);
    }
}
fn verify_interrupted(root: &Path, fixture: &Path, namespace: &str, lookup: &str, signal: &str) {
    let receipt = receipt_for(root, namespace);
    let created = root.join("destination").exists();
    let output = command()
        .args([
            "files",
            lookup,
            "receipt",
            "--operation-id",
            receipt["operation_id"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(envelope(&output.stdout, "files")["Ok"]["result"], receipt);
    super::mutation::cleanup(&receipt);
    assert!(!created, "{lookup} worker wrote after parent {signal}");
    assert_eq!(receipt["status"], "uncertain");
    assert_eq!(receipt["completion_verified"], false);
    assert_eq!(
        fs::read(root.join("source")).unwrap(),
        b"owned source bytes"
    );
    let progress = fs::read_to_string(fixture.join("progress")).unwrap();
    assert!(
        progress.contains(receipt["operation_id"].as_str().unwrap()),
        "operation ID must be published before dispatch"
    );
}
