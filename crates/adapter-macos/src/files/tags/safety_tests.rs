//! Refuse stale-snapshot replacements; only absence can be guarded atomically.
use super::tests::{direct, fixture, refresh};
use super::*;
use cueward_core::files::tags::TagEdit;

#[test]
fn native_replacement_cannot_overwrite_an_edit_after_the_last_observation() {
    let (_root, request) = fixture();
    let file = context::Context::prepare(&request.root, &request.path, None)
        .unwrap()
        .file;
    let tags = |name: &str| {
        vec![FileTag {
            name: name.into(),
            color: Some(6),
        }]
    };
    native::write(&file, &codec::encode(&tags("Original")).unwrap(), false).unwrap();
    let observed = native::read(&file).unwrap();
    let planned = edit_tags(&observed.tags, &request.edit).unwrap();
    // This owned external edit occurs AFTER the last read, directly before the setter.
    let external = tags("Finder edit");
    let bytes = codec::encode(&external).unwrap();
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    assert!(
        std::process::Command::new("/usr/bin/xattr")
            .args(["-wx", "com.apple.metadata:_kMDItemUserTags", &hex])
            .arg(request.root.join(&request.path))
            .status()
            .unwrap()
            .success()
    );
    let result = native::write(&file, &codec::encode(&planned).unwrap(), true);
    assert_eq!(
        native::read(&file).unwrap().tags,
        external,
        "the external edit must survive"
    );
    assert!(
        result.is_err(),
        "a stale replacement must never be submitted"
    );
    assert_eq!(native::read(&file).unwrap().raw, Some(bytes));
}

#[test]
fn mutating_existing_tag_attributes_is_rejected_before_submission() {
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
        assert_eq!(receipt.status, TagsStatus::NotStarted);
        assert!(!receipt.mutation_attempted && !receipt.completion_verified);
        assert_eq!(receipt.error.unwrap().code, FileErrorCode::UnsupportedType);
        assert_eq!(
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
    assert_eq!(receipt.status, TagsStatus::NotStarted);
    assert!(!receipt.mutation_attempted);
    assert_eq!(receipt.error.unwrap().code, FileErrorCode::UnsupportedType);
    assert_eq!(native::read(&file).unwrap().raw, before);
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
