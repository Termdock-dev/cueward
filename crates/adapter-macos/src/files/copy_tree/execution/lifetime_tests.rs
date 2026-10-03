//! Parent death and deadlines stop the real worker even after private payload progress.
use super::*;
const ENV: &str = "CUEWARD_TEST_TREE_LIFETIME_CONFIG";
const PREFIX: &str = "files::copy_tree::execution::safety_tests::lifetime::";

pub(super) use crate::files::lifetime_test_support::wait_for;
use crate::files::lifetime_test_support::{config, launch};

#[test]
fn tree_parent_helper() {
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
fn tree_worker_helper() {
    let Some((control, value)) = config(ENV) else {
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
fn interrupted(mode: &str, signal: Option<&str>) {
    let root = fixture();
    let control = tempfile::tempdir().unwrap();
    let mut selected = request(root.path());
    if mode == "package" {
        fs::create_dir(root.path().join("source/Owned.app")).unwrap();
        fs::write(
            root.path().join("source/Owned.app/payload"),
            b"OWNED package bytes",
        )
        .unwrap();
        selected = request(root.path());
        selected.include_packages = true;
    }
    let source_version = selected.expected_version.clone();
    let mut processes = launch(
        control.path(),
        serde_json::json!({"request": &selected, "mode": mode,
            "timeout": if signal.is_some() { 30000 } else { 2000 }}),
        ENV,
        &format!("{PREFIX}tree_parent_helper"),
        &format!("{PREFIX}tree_worker_helper"),
    );
    let id = processes.interrupt(control.path(), signal);
    let receipt = read_receipt(&id).unwrap();
    assert_eq!(receipt.status, MutationStatus::Uncertain);
    assert!(!receipt.completion_verified && !receipt.mutation_attempted);
    assert_eq!(receipt.destination_created, None);
    assert!(!root.path().join("destination").exists());
    assert_eq!(request(root.path()).expected_version, source_version);
    assert_eq!(
        fs::read(root.path().join("source/.hidden")).unwrap(),
        b"OWNED hidden\0"
    );
    if mode == "partial" || mode == "package" {
        assert_eq!(receipt.stage, TreeStage::Copying);
        let staged = Path::new(receipt.staging_path.as_ref().unwrap());
        assert_eq!(fs::read(staged.join(".hidden")).unwrap(), b"OWNED hidden\0");
        assert!(!receipt.staging_verified);
    } else {
        assert_eq!(receipt.stage, TreeStage::Preflight);
        assert!(receipt.staging_path.is_none());
    }
    if mode == "package" {
        assert!(receipt.request.include_packages);
        assert_eq!(
            fs::read(root.path().join("source/Owned.app/payload")).unwrap(),
            b"OWNED package bytes"
        );
    }
    let directory = Path::new(&receipt.receipt_path).parent().unwrap();
    assert_eq!(directory.file_name().unwrap().to_str().unwrap(), id);
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn tree_execute_parent_termination_stops_armed_and_partially_copied_workers() {
    for mode in ["armed", "partial"] {
        interrupted(mode, Some("-TERM"));
    }
}
#[test]
fn tree_execute_deadline_after_private_copy_preserves_uncertain_receipt() {
    interrupted("partial", None);
}

#[test]
fn package_execute_parent_termination_and_deadline_preserve_partial_private_copy_without_publication()
 {
    interrupted("package", Some("-TERM"));
    interrupted("package", None);
}
