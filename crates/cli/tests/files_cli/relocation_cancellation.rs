//! Parent interruption must prevent a delayed writer from creating its destination.
use super::*;
use cueward_adapter_macos::files::relocation::{self, RelocationAction, RelocationRequest};

#[test]
fn cancellation_parent_helper() {
    let Some(config) = std::env::var_os("CUEWARD_TEST_CANCEL_CONFIG") else {
        return;
    };
    let value: Value = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    let request: RelocationRequest = serde_json::from_value(value["request"].clone()).unwrap();
    let _ = relocation::run(
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
    let request = relocation::read_supervised_request().unwrap();
    fs::write(directory.join("ready"), std::process::id().to_string()).unwrap();
    wait_for(|| directory.join("release").exists());
    // If the lifetime monitor fails, this would perform the actual filesystem mutation.
    relocation::execute_worker(&request).unwrap();
}

use super::cancellation::wait_for;
fn prepare(root: &Path, action: &str) -> RelocationRequest {
    let link_itself = action == "link";
    if link_itself {
        fs::write(root.join("owned-target"), b"owned source bytes").unwrap();
        std::os::unix::fs::symlink("owned-target", root.join("source")).unwrap();
    } else {
        fs::write(root.join("source"), b"owned source bytes").unwrap();
    }
    let parent = super::mutation::version(root, ".");
    let action = if action == "rename" {
        RelocationAction::Rename
    } else {
        RelocationAction::Move
    };
    RelocationRequest {
        root: root.into(),
        path: "source".into(),
        expected_version: super::mutation::version(root, "source"),
        destination: "destination".into(),
        expected_parent_version: parent,
        action,
        link_itself,
    }
}
fn interrupt_delayed(action: &str, signal: &str, armed: bool) {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("root");
    fs::create_dir(&root).unwrap();
    let request = prepare(&root, action);
    let target_version = request
        .link_itself
        .then(|| super::mutation::version(&root, "owned-target"));
    let link_version = request.expected_version.clone();
    super::cancellation_harness::Harness {
        module: "relocation_cancellation",
        worker: "files-relocation-worker",
        namespace: "files-relocation",
        lookup: "relocation",
    }
    .interrupt(fixture.path(), &root, &request, signal, armed);
    if let Some(before) = target_version {
        assert_eq!(super::mutation::version(&root, "owned-target"), before);
        assert_eq!(super::mutation::version(&root, "source"), link_version);
        assert_eq!(
            fs::read_link(root.join("source")).unwrap(),
            Path::new("owned-target")
        );
    }
}

#[test]
fn parent_interruption_prevents_delayed_rename_and_move() {
    for action in ["rename", "move"] {
        for signal in ["-INT", "-TERM", "-KILL"] {
            interrupt_delayed(action, signal, false);
        }
    }
}

#[test]
fn parent_death_stops_an_armed_mutation_worker() {
    for action in ["rename", "move"] {
        for signal in ["-INT", "-TERM", "-KILL"] {
            interrupt_delayed(action, signal, true);
        }
    }
}

#[test]
fn link_parent_signals_stop_delayed_and_armed_workers_without_moving_link_or_target() {
    for armed in [false, true] {
        for signal in ["-INT", "-TERM", "-KILL"] {
            interrupt_delayed("link", signal, armed);
        }
    }
}
