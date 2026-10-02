//! The Creating checkpoint cannot authorize writes through a parent moved outside root.
use super::*;

fn moved_parent(action: MutationAction) {
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
            moved = true;
        }
        Ok(())
    });
    if let Err(error) = result {
        record_error(&mut receipt, error);
    }
    journal::save(&receipt).unwrap();
    let saved = journal::load(&receipt.operation_id).unwrap();
    fs::remove_dir_all(journal::directory(&receipt.operation_id).unwrap()).unwrap();
    assert!(moved, "must move after the last pre-create check");
    assert_eq!(fs::read(root.path().join("source.txt")).unwrap(), source);
    assert!(
        !outside.path().join("moved/destination").exists(),
        "created an object outside selected root: {:?}",
        saved.status
    );
    assert!(!saved.completion_verified);
}
#[test]
fn moving_parent_outside_root_before_copy_cannot_publish_source_bytes() {
    moved_parent(MutationAction::Copy {
        path: "source.txt".into(),
        expected_version: "reobserved".into(),
        max_bytes: 1024,
    });
}
#[test]
fn moving_parent_outside_root_before_mkdir_cannot_create_a_directory() {
    moved_parent(MutationAction::Mkdir);
}
