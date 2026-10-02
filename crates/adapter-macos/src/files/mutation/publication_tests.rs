use super::*;

#[test]
fn unsupported_or_ignored_publication_flags_fail_closed() {
    let (root, request) = fixture();
    let prepared = journal::create(&request).unwrap();
    let path = journal::directory(&prepared.operation_id).unwrap();
    let directory = MacFiles.open_directory(&path).unwrap();
    for flags in [0, 0x04, 0x40000000] {
        assert_eq!(
            publication::probe(&directory, &path, flags)
                .unwrap_err()
                .code,
            FileErrorCode::Unavailable
        );
    }
    assert!(!root.path().join("copy.txt").exists());
    assert!(!path.join("staged-object").exists());
    assert_eq!(
        fs::read_dir(&path).unwrap().count(),
        1,
        "only the receipt remains after empty probes"
    );
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn a_different_root_filesystem_is_rejected_before_staging() {
    let (_root, request) = fixture();
    let prepared = journal::create(&request).unwrap();
    let path = journal::directory(&prepared.operation_id).unwrap();
    let other = MacFiles.open_directory(Path::new("/dev")).unwrap();
    assert_ne!(
        other.metadata().unwrap().dev(),
        fs::metadata(&path).unwrap().dev()
    );
    assert_eq!(
        publication::prepare(&prepared, &other).err().unwrap().code,
        FileErrorCode::Unavailable
    );
    assert!(!path.join("staged-object").exists());
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn old_receipts_without_staging_fields_remain_readable() {
    let (_root, request) = fixture();
    let prepared = journal::create(&request).unwrap();
    let mut value = serde_json::to_value(&prepared).unwrap();
    value.as_object_mut().unwrap().remove("staging_path");
    value.as_object_mut().unwrap().remove("staging_verified");
    fs::write(&prepared.receipt_path, serde_json::to_vec(&value).unwrap()).unwrap();
    let loaded = journal::load(&prepared.operation_id).unwrap();
    assert!(loaded.staging_path.is_none());
    assert!(!loaded.staging_verified);
    fs::remove_dir_all(journal::directory(&prepared.operation_id).unwrap()).unwrap();
}
#[test]
fn failed_staging_checkpoint_never_publishes_even_an_empty_destination() {
    let (root, request) = fixture();
    let mut receipt = journal::create(&request).unwrap();
    let source = fs::read(root.path().join("source.txt")).unwrap();
    let error = cueward_core::files::mutation::execute(&MacFiles, &mut receipt, &mut |value| {
        journal::save(value)?;
        if value.stage == MutationStage::Copying {
            return Err(FileError::new(
                FileErrorCode::Io,
                "owned staging checkpoint failure",
            ));
        }
        Ok(())
    })
    .unwrap_err();
    record_error(&mut receipt, error);
    assert_eq!(receipt.status, MutationStatus::NotStarted);
    assert!(!receipt.mutation_attempted);
    assert!(!receipt.staging_verified);
    assert!(!root.path().join("copy.txt").exists());
    assert_eq!(fs::read(root.path().join("source.txt")).unwrap(), source);
    assert_eq!(
        fs::read(receipt.staging_path.as_ref().unwrap()).unwrap(),
        b""
    );
    fs::remove_dir_all(journal::directory(&receipt.operation_id).unwrap()).unwrap();
}
