//! Parent death and deadlines stop the real worker even after private payload progress.
use super::super::execution::tests::{self as trash, Controlled};
use super::tests::*;
use super::*;
use std::fs;
use std::path::PathBuf;
const ENV: &str = "CUEWARD_TEST_RESTORE_LIFETIME_CONFIG";
const PREFIX: &str = "files::trash::restore::lifetime_tests::";

pub(super) use crate::files::lifetime_test_support::wait_for;
use crate::files::lifetime_test_support::{config, launch};

#[test]
fn restore_parent_helper() {
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
fn restore_worker_helper() {
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
                "publish" => 3,
                "published" => 4,
                _ => 0,
            },
        };
        platform.pause = Some(control);
        execute_prepared(&platform, &input).unwrap();
    }
}
fn interrupted(mode: &str, signal: Option<&str>) {
    let value = fixture();
    let original = original_json(&value);
    let control = tempfile::tempdir().unwrap();
    let mut processes = launch(
        control.path(),
        serde_json::json!({"request": &value.receipt.request, "mode": mode,
            "timeout": if signal.is_some() { 30000 } else { 2000 }, "trash": value.original.trash.path()}),
        ENV,
        &format!("{PREFIX}restore_parent_helper"),
        &format!("{PREFIX}restore_worker_helper"),
    );
    let id = processes.interrupt(control.path(), signal);
    let receipt = read_receipt(&id).unwrap();
    assert_eq!(receipt.status, MutationStatus::Uncertain);
    assert!(!receipt.completion_verified);
    assert_eq!(receipt.destination_created, None);
    let path = value.original.root.path().join("source");
    if mode == "published" {
        assert!(receipt.mutation_attempted && receipt.staging_verified);
        assert_eq!(receipt.stage, RestoreStage::Publishing);
        assert_eq!(fs::read(&path).unwrap(), trash::backup(&value.original));
    } else {
        assert!(!path.exists());
        if mode == "armed" {
            assert_eq!(receipt.stage, RestoreStage::Preflight);
            assert!(receipt.staging_path.is_none());
        } else {
            assert_eq!(
                fs::read(receipt.staging_path.as_ref().unwrap()).unwrap(),
                trash::backup(&value.original)
            );
            if mode == "partial" {
                assert_eq!(receipt.stage, RestoreStage::Copying);
                assert!(!receipt.staging_verified);
            } else {
                assert_eq!(receipt.stage, RestoreStage::Publishing);
                assert!(receipt.staging_verified);
            }
        }
    }
    assert_retained(&value, &original);
    let directory = Path::new(&receipt.receipt_path).parent().unwrap();
    assert_eq!(directory.file_name().unwrap().to_str().unwrap(), id);
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn restore_parent_termination_stops_armed_copying_and_publication_workers_without_cleanup() {
    for mode in ["armed", "partial", "publish", "published"] {
        interrupted(mode, Some("-TERM"));
    }
}
#[test]
fn restore_deadline_preserves_uncertain_copy_and_published_destination() {
    for mode in ["partial", "publish", "published"] {
        interrupted(mode, None);
    }
}
