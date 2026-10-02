//! Parent interruption must prevent a delayed writer from creating its destination.
use super::*;
use cueward_adapter_macos::files::batch_execution;
use cueward_adapter_macos::files::batch_rename::BatchRenameRequest;

#[test]
fn cancellation_parent_helper() {
    let Some(config) = std::env::var_os("CUEWARD_TEST_CANCEL_CONFIG") else {
        return;
    };
    let value: Value = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    let request: BatchRenameRequest = serde_json::from_value(value["request"].clone()).unwrap();
    let _ = batch_execution::run(
        Path::new(value["worker"].as_str().unwrap()),
        &request,
        30000,
    );
}

#[test]
fn cancellation_worker_helper() {
    let Some(config) = std::env::var_os("CUEWARD_TEST_CANCEL_CONFIG") else {
        return;
    };
    let config = Path::new(&config);
    let directory = config.parent().unwrap();
    let request = batch_execution::read_supervised_request().unwrap();
    fs::write(directory.join("ready"), std::process::id().to_string()).unwrap();
    wait_for(|| directory.join("release").exists());
    // If the lifetime monitor fails, this would perform the actual filesystem mutation.
    batch_execution::execute_worker(&request).unwrap();
}

use super::cancellation::wait_for;
fn prepare(root: &Path, _action: &str) -> BatchRenameRequest {
    fs::write(root.join("source"), b"owned source bytes").unwrap();
    BatchRenameRequest {
        root: root.into(),
        entries: vec![
            serde_json::from_str(&super::batch_rename::entry(root, "source", "destination"))
                .unwrap(),
        ],
    }
}

fn interrupt_delayed(action: &str, signal: &str, armed: bool) {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("root");
    fs::create_dir(&root).unwrap();
    let request = prepare(&root, action);
    super::cancellation_harness::Harness {
        module: "batch_execution_cancellation",
        worker: "files-batch-rename-worker",
        namespace: "files-batch-rename",
        lookup: "rename-batch",
    }
    .interrupt(fixture.path(), &root, &request, signal, armed);
}

#[test]
fn batch_execute_parent_interruption_prevents_delayed_writer() {
    for action in ["batch rename"] {
        for signal in ["-INT", "-TERM", "-KILL"] {
            interrupt_delayed(action, signal, false);
        }
    }
}

#[test]
fn batch_execute_parent_death_stops_armed_writer() {
    for action in ["batch rename"] {
        for signal in ["-INT", "-TERM", "-KILL"] {
            interrupt_delayed(action, signal, true);
        }
    }
}
