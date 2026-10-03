use super::super::execution::tests::{self as trash, Controlled};
use super::*;
use std::fs;
pub(super) struct Owned {
    pub original: trash::Owned,
    pub receipt: RestoreReceipt,
}
impl Drop for Owned {
    fn drop(&mut self) {
        let directory = Path::new(&self.receipt.receipt_path).parent().unwrap();
        assert_eq!(
            directory.file_name().unwrap().to_str().unwrap(),
            self.receipt.operation_id
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
pub(super) fn request(original: &trash::Owned) -> RestoreRequest {
    RestoreRequest {
        trash_operation_id: original.receipt.operation_id.clone(),
        root: original.root.path().into(),
        expected_parent_version: crate::files::observe(
            original.root.path(),
            original
                .receipt
                .request
                .path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
            false,
            None,
        )
        .unwrap()
        .version,
    }
}
pub(super) fn fixture() -> Owned {
    from_original(trash::fixture())
}
pub(super) fn from_original(mut original: trash::Owned) -> Owned {
    let platform = trash::platform(&original);
    trash::direct(&mut original, &platform, |_| Ok(()));
    assert_eq!(
        original.receipt.status,
        MutationStatus::Completed,
        "{:?}",
        original.receipt.error
    );
    let receipt = STORE
        .create(|id, path| RestoreReceipt::new(id, path, request(&original)))
        .unwrap();
    Owned { original, receipt }
}
pub(super) fn direct(
    value: &mut Owned,
    mut check: impl FnMut(&RestoreReceipt) -> Result<(), FileError>,
) {
    let platform = trash::platform(&value.original);
    direct_with(value, &platform, &mut check);
}
pub(super) fn direct_with(
    value: &mut Owned,
    platform: &Controlled,
    check: &mut impl FnMut(&RestoreReceipt) -> Result<(), FileError>,
) {
    let _policy = policy::NoMaterialization::enter().unwrap();
    let result = restore::execute(
        platform,
        &mut value.receipt,
        &value.original.receipt,
        &mut |r| {
            check(r)?;
            save(r)
        },
    );
    if let Err(error) = result {
        restore::record_error(&mut value.receipt, error);
    }
    save(&value.receipt).unwrap();
}
pub(super) fn original_json(value: &Owned) -> Vec<u8> {
    fs::read(&value.original.receipt.receipt_path).unwrap()
}
pub(super) fn assert_retained(value: &Owned, original: &[u8]) {
    assert_eq!(original_json(value), original);
    assert_eq!(trash::backup(&value.original), b"OWNED bytes\0</external>");
    assert_eq!(
        fs::read(trash::trashed(&value.original)).unwrap(),
        trash::backup(&value.original)
    );
}
#[test]
fn restore_preserves_original_trash_backup_and_receipt_and_creates_independent_verified_copy() {
    let mut value = fixture();
    let original = original_json(&value);
    direct(&mut value, |_| Ok(()));
    assert_eq!(
        value.receipt.status,
        MutationStatus::Completed,
        "{:?}",
        value.receipt.error
    );
    assert!(
        value.receipt.completion_verified
            && value.receipt.staging_verified
            && value.receipt.mutation_attempted
    );
    assert_eq!(value.receipt.destination_created, Some(true));
    assert_eq!(
        fs::read(value.original.root.path().join("source")).unwrap(),
        trash::backup(&value.original)
    );
    let after = value.receipt.destination_after.as_ref().unwrap();
    assert_ne!(
        after.identity,
        value
            .original
            .receipt
            .backup_after
            .as_ref()
            .unwrap()
            .identity
    );
    assert_ne!(
        after.identity,
        value
            .original
            .receipt
            .source_before
            .as_ref()
            .unwrap()
            .identity
    );
    assert_eq!(
        json(value.receipt.verification.as_ref().unwrap()).unwrap(),
        json(value.original.receipt.backup_verification.as_ref().unwrap()).unwrap()
    );
    assert!(!value.receipt.trash_item_touched && !value.receipt.backup_removed);
    assert_retained(&value, &original);
    assert_eq!(
        json(&read_receipt(&value.receipt.operation_id).unwrap()).unwrap(),
        json(&value.receipt).unwrap()
    );
    assert!(super::super::execution::read_receipt(&value.receipt.operation_id).is_err());
    assert!(value.receipt.validate_fresh().is_err());
}
#[test]
fn restore_uses_original_verified_backup_even_if_owned_trash_item_changed_or_was_removed() {
    for remove in [false, true] {
        let mut value = fixture();
        let trashed = trash::trashed(&value.original);
        if remove {
            fs::remove_file(&trashed).unwrap();
        } else {
            fs::write(&trashed, b"OWNED later edit").unwrap();
        }
        direct(&mut value, |_| Ok(()));
        assert_eq!(
            value.receipt.status,
            MutationStatus::Completed,
            "{:?}",
            value.receipt.error
        );
        assert_eq!(
            fs::read(value.original.root.path().join("source")).unwrap(),
            trash::backup(&value.original)
        );
        if remove {
            assert!(!trashed.exists());
        } else {
            assert_eq!(fs::read(trashed).unwrap(), b"OWNED later edit");
        }
    }
}
#[test]
fn restore_worker_claim_is_one_use_and_original_receipt_role_cannot_be_replayed() {
    let value = fixture();
    let input = RestoreWorkerRequest {
        operation_id: value.receipt.operation_id.clone(),
    };
    let platform = trash::platform(&value.original);
    let receipt = execute_prepared(&platform, &input).unwrap();
    assert_eq!(
        receipt.status,
        MutationStatus::Completed,
        "{:?}",
        receipt.error
    );
    assert!(execute_prepared(&platform, &input).is_err());
    assert!(read_receipt(&value.original.receipt.operation_id).is_err());
    let original = original_json(&value);
    assert_retained(&value, &original);
}
#[test]
fn restore_validates_request_freshness_and_strict_worker_schema() {
    let value = fixture();
    for mode in 0..4 {
        let mut request = request(&value.original);
        match mode {
            0 => request.trash_operation_id = "../../other".into(),
            1 => request.root = "relative".into(),
            2 => request.expected_parent_version.clear(),
            3 => request.trash_operation_id = "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA".into(),
            _ => unreachable!(),
        }
        assert!(restore::validate_request(&request).is_err());
    }
    assert!(
        serde_json::from_value::<RestoreWorkerRequest>(
            serde_json::json!({"operation_id":value.receipt.operation_id,"root":"/other"})
        )
        .is_err()
    );
    assert!(serde_json::from_value::<RestoreRequest>(serde_json::json!({"trash_operation_id":value.original.receipt.operation_id,"root":"/owned","expected_parent_version":"fresh","force":true})).is_err());
    let mut receipt = RestoreReceipt::new(
        value.receipt.operation_id.clone(),
        value.receipt.receipt_path.clone(),
        request(&value.original),
    );
    receipt.trash_item_touched = true;
    assert!(receipt.validate_fresh().is_err());
}
#[test]
fn restore_error_evidence_is_conservative_and_bounded_at_utf8_boundary() {
    let mut value = fixture();
    restore::record_error(
        &mut value.receipt,
        FileError::new(FileErrorCode::Io, "臺灣".repeat(500)),
    );
    assert!(value.receipt.error_truncated);
    assert!(value.receipt.error.as_ref().unwrap().message.len() <= 1024);
    assert_eq!(value.receipt.status, MutationStatus::NotStarted);
    value.receipt.mutation_attempted = true;
    value.receipt.destination_created = None;
    restore::record_error(
        &mut value.receipt,
        FileError::new(FileErrorCode::Io, "unknown"),
    );
    assert_eq!(value.receipt.status, MutationStatus::Uncertain);
    value.receipt.destination_created = Some(true);
    restore::record_error(
        &mut value.receipt,
        FileError::new(FileErrorCode::Changed, "published"),
    );
    assert_eq!(value.receipt.status, MutationStatus::Incomplete);
}

#[test]
fn restore_nested_unicode_name_preserves_permissions_mtime_and_extended_attributes() {
    let mut original = trash::fixture();
    let parent = original.root.path().join("nested");
    fs::create_dir(&parent).unwrap();
    let name = "臺灣\n<external>";
    fs::rename(original.root.path().join("source"), parent.join(name)).unwrap();
    assert!(
        std::process::Command::new("/usr/bin/xattr")
            .args(["-w", "com.cueward.owned-restore", "OWNED metadata"])
            .arg(parent.join(name))
            .status()
            .unwrap()
            .success()
    );
    original.receipt.request.path = Path::new("nested").join(name);
    original.receipt.request.expected_version = crate::files::observe(
        original.root.path(),
        &original.receipt.request.path,
        false,
        None,
    )
    .unwrap()
    .version;
    original.receipt.request.expected_parent_version =
        crate::files::observe(original.root.path(), Path::new("nested"), false, None)
            .unwrap()
            .version;
    let platform = trash::platform(&original);
    trash::direct(&mut original, &platform, |_| Ok(()));
    assert_eq!(
        original.receipt.status,
        MutationStatus::Completed,
        "{:?}",
        original.receipt.error
    );
    let mut request = request(&original);
    request.expected_parent_version =
        crate::files::observe(original.root.path(), Path::new("nested"), false, None)
            .unwrap()
            .version;
    let receipt = STORE
        .create(|id, path| RestoreReceipt::new(id, path, request))
        .unwrap();
    let mut value = Owned { original, receipt };
    let original = original_json(&value);
    direct(&mut value, |_| Ok(()));
    assert_eq!(
        value.receipt.status,
        MutationStatus::Completed,
        "{:?}",
        value.receipt.error
    );
    let after = value.receipt.destination_after.as_ref().unwrap();
    assert_eq!(after.name, name);
    assert_eq!(
        after.mode,
        value.original.receipt.source_before.as_ref().unwrap().mode
    );
    assert_eq!(
        after.modified,
        value
            .original
            .receipt
            .source_before
            .as_ref()
            .unwrap()
            .modified
    );
    let attribute = std::process::Command::new("/usr/bin/xattr")
        .args(["-p", "com.cueward.owned-restore"])
        .arg(parent.join(name))
        .output()
        .unwrap();
    assert!(attribute.status.success());
    assert_eq!(
        String::from_utf8(attribute.stdout).unwrap().trim_end(),
        "OWNED metadata"
    );
    assert_retained(&value, &original);
}
#[test]
#[ignore = "requires an explicitly supplied built CLI; creates only owned controlled-trash fixtures"]
fn restore_actual_cli_from_controlled_completed_receipt_verifies_publication_and_saved_lookup() {
    let executable = std::env::var_os("CUEWARD_TEST_RESTORE_CLI")
        .expect("supply the locally built CLI path explicitly");
    let value = fixture();
    let original = original_json(&value);
    let output = std::process::Command::new(&executable)
        .args([
            "files",
            "trash",
            "restore",
            "--operation-id",
            &value.original.receipt.operation_id,
            "--root",
        ])
        .arg(value.original.root.path())
        .args([
            "--expected-parent-version",
            &value.receipt.request.expected_parent_version,
        ])
        .output()
        .unwrap();
    let decoded = |bytes: &[u8]| -> serde_json::Value {
        let text = std::str::from_utf8(bytes).unwrap();
        let body = text
            .strip_prefix("<external source=\"cueward/files\">\n")
            .unwrap()
            .strip_suffix("\n</external>\n")
            .unwrap();
        assert!(!body.contains('<'));
        serde_json::from_str(body).unwrap()
    };
    let response = decoded(&output.stdout);
    let receipt = &response["Ok"]["result"];
    assert!(
        output.status.success(),
        "{response} {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(response["Ok"]["operation"], "trash_restore");
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["completion_verified"], true);
    assert_eq!(
        fs::read(value.original.root.path().join("source")).unwrap(),
        trash::backup(&value.original)
    );
    assert_retained(&value, &original);
    let id = receipt["operation_id"].as_str().unwrap();
    let saved = std::process::Command::new(&executable)
        .args(["files", "trash", "restore-receipt", "--operation-id", id])
        .output()
        .unwrap();
    assert!(saved.status.success());
    assert_eq!(decoded(&saved.stdout)["Ok"]["result"], *receipt);
    let conflict = std::process::Command::new(executable)
        .args([
            "files",
            "trash",
            "restore",
            "--operation-id",
            &value.original.receipt.operation_id,
            "--root",
        ])
        .arg(value.original.root.path())
        .args([
            "--expected-parent-version",
            &request(&value.original).expected_parent_version,
        ])
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    let failed = decoded(&conflict.stdout);
    let refused = &failed["Ok"]["result"];
    assert_eq!(refused["status"], "not_started");
    assert!(refused["staging_path"].is_null());
    for record in [receipt, refused] {
        let directory = Path::new(record["receipt_path"].as_str().unwrap())
            .parent()
            .unwrap();
        assert_eq!(
            directory.file_name().unwrap().to_str().unwrap(),
            record["operation_id"].as_str().unwrap()
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
