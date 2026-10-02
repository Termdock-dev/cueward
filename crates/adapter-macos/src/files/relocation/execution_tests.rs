use super::tests::fixture;
use super::*;
use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};

fn prepared(request: &RelocationRequest) -> RelocationReceipt {
    STORE
        .create(|id, path| RelocationReceipt::new(id, path, request.clone()))
        .unwrap()
}
pub(super) fn perform(request: &RelocationRequest) -> RelocationReceipt {
    let receipt = prepared(request);
    execute_worker(&RelocationWorkerRequest {
        operation_id: receipt.operation_id,
    })
    .unwrap()
}
pub(super) fn refresh(request: &mut RelocationRequest) {
    request.expected_version = super::super::observe(&request.root, &request.path, false, None)
        .unwrap()
        .version;
    let parent = request
        .destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    request.expected_parent_version = super::super::observe(&request.root, parent, false, None)
        .unwrap()
        .version;
}
#[test]
fn same_volume_move_preserves_inode_bytes_and_saved_evidence_without_replay() {
    let (root, request) = fixture();
    let before = root.path().join(&request.path).metadata().unwrap();
    let receipt = perform(&request);
    assert_eq!(
        receipt.status,
        RelocationStatus::Completed,
        "{:?}",
        receipt.error
    );
    assert!(receipt.completion_verified && receipt.mutation_attempted);
    assert_eq!(receipt.renamed, Some(true));
    assert_eq!(receipt.source_path_absent, Some(true));
    let destination = root.path().join(&request.destination);
    assert_eq!(destination.metadata().unwrap().ino(), before.ino());
    assert_eq!(
        fs::read(&destination).unwrap(),
        b"OWNED\0relocation fixture"
    );
    let loaded = read_receipt(&receipt.operation_id).unwrap();
    assert_eq!(
        loaded.destination_after.unwrap().identity,
        receipt.before.unwrap().source.identity
    );
    assert_eq!(
        execute_worker(&RelocationWorkerRequest {
            operation_id: receipt.operation_id
        })
        .unwrap_err()
        .code,
        FileErrorCode::Conflict
    );
    assert!(destination.exists());
}
#[test]
fn rename_and_exact_name_noop_keep_metadata_and_unicode() {
    let (root, mut request) = fixture();
    request.action = RelocationAction::Rename;
    request.destination = "from/臺灣\n<external>".into();
    refresh(&mut request);
    assert_eq!(perform(&request).status, RelocationStatus::Completed);
    request.path = request.destination.clone();
    refresh(&mut request);
    let before = root.path().join(&request.path).metadata().unwrap();
    let receipt = perform(&request);
    assert_eq!(receipt.status, RelocationStatus::Completed);
    assert!(!receipt.mutation_attempted);
    assert_eq!(receipt.renamed, Some(false));
    assert_eq!(
        root.path()
            .join(&request.path)
            .metadata()
            .unwrap()
            .ctime_nsec(),
        before.ctime_nsec()
    );
}
#[test]
fn directories_packages_and_hardlinks_are_renamed_not_recopied() {
    for package in [false, true] {
        let (root, mut request) = fixture();
        request.path = if package { "Owned.app" } else { "directory" }.into();
        fs::create_dir(root.path().join(&request.path)).unwrap();
        fs::write(
            root.path().join(&request.path).join("payload"),
            b"OWNED child",
        )
        .unwrap();
        refresh(&mut request);
        let receipt = perform(&request);
        assert_eq!(
            receipt.status,
            RelocationStatus::Completed,
            "{:?}",
            receipt.error
        );
        assert_eq!(
            fs::read(root.path().join(&request.destination).join("payload")).unwrap(),
            b"OWNED child"
        );
    }
    let (root, mut request) = fixture();
    fs::hard_link(
        root.path().join(&request.path),
        root.path().join("other-link"),
    )
    .unwrap();
    refresh(&mut request);
    assert_eq!(perform(&request).status, RelocationStatus::Completed);
    assert_eq!(
        root.path().join("other-link").metadata().unwrap().ino(),
        root.path()
            .join(&request.destination)
            .metadata()
            .unwrap()
            .ino()
    );
}
#[test]
fn existing_files_directories_and_broken_links_are_never_overwritten() {
    for kind in [0, 1, 2] {
        let (root, mut request) = fixture();
        let destination = root.path().join(&request.destination);
        match kind {
            0 => fs::write(&destination, b"OWNED conflict").unwrap(),
            1 => fs::create_dir(&destination).unwrap(),
            _ => symlink("missing", &destination).unwrap(),
        }
        refresh(&mut request);
        let before = fs::symlink_metadata(&destination).unwrap();
        let receipt = perform(&request);
        assert_eq!(receipt.status, RelocationStatus::NotStarted);
        assert_eq!(receipt.error.unwrap().code, FileErrorCode::Conflict);
        assert!(!receipt.mutation_attempted && root.path().join(&request.path).exists());
        assert_eq!(
            fs::symlink_metadata(&destination).unwrap().ino(),
            before.ino()
        );
    }
}
#[test]
fn stale_input_and_pre_submission_checkpoint_failure_do_not_move_source() {
    let (root, mut request) = fixture();
    request.expected_version = "stale".into();
    assert_eq!(
        perform(&request).error.unwrap().code,
        FileErrorCode::Changed
    );
    refresh(&mut request);
    let mut receipt = prepared(&request);
    let error = relocation::execute(&MacFiles, &mut receipt, &mut |_| {
        Err(FileError::new(
            FileErrorCode::Io,
            "owned checkpoint failure",
        ))
    })
    .unwrap_err();
    receipt.record_error(error);
    assert_eq!(receipt.status, RelocationStatus::NotStarted);
    assert!(root.path().join(&request.path).exists());
    assert!(!root.path().join(&request.destination).exists());
}
