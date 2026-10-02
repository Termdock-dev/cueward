//! The Creating checkpoint cannot authorize writes through a parent moved outside root.
use super::*;

fn moved_parent(action: MutationAction, replace_with_link: bool) {
    let (root, mut request) = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("nested/parent")).unwrap();
    request.destination = "nested/parent/destination".into();
    request.action = action;
    fresh(&mut request);
    let source = fs::read(root.path().join("source.txt")).unwrap();
    let mut receipt = journal::create(&request).unwrap();
    let mut moved = false;
    let result = cueward_core::files::mutation::execute(&MacFiles, &mut receipt, &mut |value| {
        journal::save(value)?;
        if value.stage == MutationStage::Creating && value.destination_created.is_none() {
            fs::rename(
                root.path().join("nested/parent"),
                outside.path().join("moved"),
            )?;
            if replace_with_link {
                symlink(
                    outside.path().join("moved"),
                    root.path().join("nested/parent"),
                )?;
            }
            moved = true;
        }
        Ok(())
    });
    if let Err(error) = result {
        record_error(&mut receipt, error);
    }
    journal::save(&receipt).unwrap();
    let saved = journal::load(&receipt.operation_id).unwrap();
    check_staging(&saved, &request.action, &source);
    fs::remove_dir_all(journal::directory(&receipt.operation_id).unwrap()).unwrap();
    assert_eq!(saved.status, MutationStatus::NotStarted);
    assert_eq!(saved.destination_created, Some(false));
    assert!(moved, "must move after the last pre-create check");
    assert_eq!(fs::read(root.path().join("source.txt")).unwrap(), source);
    assert!(
        !outside.path().join("moved/destination").exists(),
        "created an object outside selected root: {:?}",
        saved.status
    );
    assert!(!saved.completion_verified);
}
fn check_staging(receipt: &MutationReceipt, action: &MutationAction, source: &[u8]) {
    assert!(receipt.staging_verified);
    let staging = Path::new(receipt.staging_path.as_ref().unwrap());
    if matches!(action, MutationAction::Copy { .. }) {
        assert_eq!(fs::read(staging).unwrap(), source);
    } else {
        assert!(staging.is_dir());
    }
}
#[test]
fn moving_parent_outside_root_before_copy_cannot_publish_source_bytes() {
    moved_parent(
        MutationAction::Copy {
            path: "source.txt".into(),
            expected_version: "reobserved".into(),
            max_bytes: 1024,
        },
        false,
    );
}
#[test]
fn moving_parent_outside_root_before_mkdir_cannot_create_a_directory() {
    moved_parent(MutationAction::Mkdir, false);
}

#[test]
fn swapping_parent_for_an_outside_symlink_before_publication_is_rejected() {
    moved_parent(
        MutationAction::Copy {
            path: "source.txt".into(),
            expected_version: "reobserved".into(),
            max_bytes: 1024,
        },
        true,
    );
}

#[test]
fn a_published_file_moved_outside_root_receives_no_further_payload_writes() {
    let (root, mut request) = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("nested/parent")).unwrap();
    request.destination = "nested/parent/destination".into();
    fresh(&mut request);
    let source = fs::read(root.path().join("source.txt")).unwrap();
    let mut receipt = journal::create(&request).unwrap();
    let mut moved = false;
    let error = cueward_core::files::mutation::execute(&MacFiles, &mut receipt, &mut |value| {
        journal::save(value)?;
        if value.destination_created == Some(true) && !moved {
            assert!(value.staging_verified);
            assert_eq!(fs::read(root.path().join(&request.destination))?, source);
            fs::rename(
                root.path().join("nested/parent"),
                outside.path().join("moved"),
            )?;
            fs::write(
                outside.path().join("moved/destination"),
                b"external replacement",
            )?;
            moved = true;
        }
        Ok(())
    })
    .unwrap_err();
    record_error(&mut receipt, error);
    assert!(moved);
    assert_eq!(receipt.status, MutationStatus::Incomplete);
    assert!(!receipt.completion_verified);
    assert_eq!(
        fs::read(outside.path().join("moved/destination")).unwrap(),
        b"external replacement"
    );
    assert_eq!(fs::read(root.path().join("source.txt")).unwrap(), source);
    fs::remove_dir_all(journal::directory(&receipt.operation_id).unwrap()).unwrap();
}
