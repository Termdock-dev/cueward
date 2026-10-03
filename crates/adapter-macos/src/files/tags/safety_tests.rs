//! Observed revision guards preserve detected external edits; replacement is explicitly non-CAS.
use super::tests::{direct, fixture, refresh};
use super::*;
use cueward_core::files::tags::TagEdit;

#[test]
fn observed_external_edit_before_submission_is_not_overwritten() {
    let (_root, mut request) = fixture();
    assert_eq!(
        direct(&request, &mut |_| Ok(())).status,
        TagsStatus::Completed
    );
    let file = context::Context::prepare(&request.root, &request.path, None)
        .unwrap()
        .file;
    request.edit = TagEdit::Add(vec!["Other".into()]);
    refresh(&mut request);
    let external = codec::encode(&[FileTag {
        name: "Finder edit".into(),
        color: Some(6),
    }])
    .unwrap();
    let receipt = direct(&request, &mut |r| {
        if r.stage == TagsStage::Writing {
            native::write(&file, &external, true)?;
        }
        Ok(())
    });
    assert_eq!(receipt.status, TagsStatus::NotStarted);
    assert_eq!(receipt.changed_by_operation, Some(false));
    assert_eq!(native::read(&file).unwrap().raw, Some(external));
}

#[test]
fn existing_tags_can_be_added_and_removed_without_losing_unselected_names() {
    let (_root, mut request) = fixture();
    assert_eq!(
        direct(&request, &mut |_| Ok(())).status,
        TagsStatus::Completed
    );
    for edit in [
        TagEdit::Add(vec!["Other".into()]),
        TagEdit::Remove(vec!["新增 <external> 臺灣".into()]),
    ] {
        request.edit = edit;
        refresh(&mut request);
        let before = read(&request.root, &request.path, None).unwrap();
        let receipt = direct(&request, &mut |_| Ok(()));
        assert_eq!(receipt.status, TagsStatus::Completed);
        assert!(receipt.mutation_attempted && receipt.completion_verified);
        assert!(receipt.original_attribute_base64.is_some());
        assert_ne!(
            read(&request.root, &request.path, None)
                .unwrap()
                .tags_version,
            before.tags_version
        );
    }
}

#[test]
fn a_present_empty_attribute_is_not_an_absent_attribute_for_initial_add() {
    let (_root, mut request) = fixture();
    let file = context::Context::prepare(&request.root, &request.path, None)
        .unwrap()
        .file;
    native::write(&file, &codec::encode(&[]).unwrap(), false).unwrap();
    refresh(&mut request);
    let before = native::read(&file).unwrap().raw;
    let receipt = direct(&request, &mut |_| Ok(()));
    assert_eq!(receipt.status, TagsStatus::Completed);
    assert!(receipt.mutation_attempted && receipt.original_attribute_base64.is_some());
    assert_ne!(native::read(&file).unwrap().raw, before);
}

#[test]
fn atomic_initial_create_preserves_an_external_attribute_created_after_observation() {
    let (_root, request) = fixture();
    let file = context::Context::prepare(&request.root, &request.path, None)
        .unwrap()
        .file;
    assert!(native::read(&file).unwrap().raw.is_none());
    let external = codec::encode(&[FileTag {
        name: "Finder edit".into(),
        color: Some(2),
    }])
    .unwrap();
    native::write(&file, &external, false).unwrap();
    let planned = codec::encode(&edit_tags(&[], &request.edit).unwrap()).unwrap();
    assert_eq!(
        native::write(&file, &planned, false).unwrap_err().code,
        FileErrorCode::Conflict
    );
    assert_eq!(native::read(&file).unwrap().raw, Some(external));
}
