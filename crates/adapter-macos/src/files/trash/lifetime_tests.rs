//! Parent death and deadlines stop the real worker even after private payload progress.
use super::tests::*;
use super::*;
use std::fs;
use std::path::PathBuf;
const ENV: &str = "CUEWARD_TEST_TRASH_LIFETIME_CONFIG";
const PREFIX: &str = "files::trash::execution::lifetime_tests::";

pub(super) use crate::files::lifetime_test_support::wait_for;
use crate::files::lifetime_test_support::{config, launch};

#[test]
fn trash_parent_helper() {
    let Some((_, value)) = config(ENV) else {
        return;
    };
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
    let Some((control, value)) = config(ENV) else {
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
fn interrupted(mode: &str, signal: Option<&str>) {
    let value = fixture();
    let root = &value.root;
    let control = tempfile::tempdir().unwrap();
    let source_version = request(root.path()).expected_version;
    let mut processes = launch(
        control.path(),
        serde_json::json!({"request": &request(root.path()), "mode": mode,
            "timeout": if signal.is_some() { 30000 } else { 2000 }, "trash": value.trash.path()}),
        ENV,
        &format!("{PREFIX}trash_parent_helper"),
        &format!("{PREFIX}trash_worker_helper"),
    );
    let id = processes.interrupt(control.path(), signal);
    let receipt = read_receipt(&id).unwrap();
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
fn trash_execute_parent_termination_stops_armed_and_partially_copied_workers() {
    for mode in ["armed", "partial", "native", "trashed"] {
        interrupted(mode, Some("-TERM"));
    }
}
#[test]
fn trash_execute_deadline_after_private_copy_preserves_uncertain_receipt() {
    for mode in ["partial", "native", "trashed"] {
        interrupted(mode, None);
    }
}
