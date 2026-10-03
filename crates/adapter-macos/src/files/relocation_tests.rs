use super::*;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

/// Allocate owned temporary source/parent observations for planning checks.
pub(super) fn fixture() -> (tempfile::TempDir, RelocationRequest) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("from")).unwrap();
    fs::create_dir(root.path().join("to")).unwrap();
    fs::write(root.path().join("from/owned"), b"OWNED\0relocation fixture").unwrap();
    let source = super::super::observe(root.path(), Path::new("from/owned"), false, None).unwrap();
    let parent = super::super::observe(root.path(), Path::new("to"), false, None).unwrap();
    let request = RelocationRequest {
        root: root.path().into(),
        path: "from/owned".into(),
        destination: "to/renamed".into(),
        expected_version: source.version,
        expected_parent_version: parent.version,
        action: RelocationAction::Move,
        link_itself: false,
    };
    (root, request)
}
#[test]
fn relocation_preparation_observes_without_moving_or_editing() {
    let (root, request) = fixture();
    let source = root.path().join(&request.path);
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    let mut request = request;
    request.expected_version = super::super::observe(root.path(), &request.path, false, None)
        .unwrap()
        .version;
    let before = source.metadata().unwrap();
    let bytes = fs::read(&source).unwrap();
    let plan = plan_worker(&request).unwrap();
    assert_eq!(plan.source.version, request.expected_version);
    assert!(plan.same_filesystem && !plan.no_op && plan.destination_before.is_none());
    assert!(plan.destination_path.ends_with("/to/renamed"));
    assert!(!root.path().join(&request.destination).exists());
    assert_eq!(fs::read(&source).unwrap(), bytes);
    let after = source.metadata().unwrap();
    assert_eq!(
        (before.ino(), before.mode(), before.mtime(), before.ctime()),
        (after.ino(), after.mode(), after.mtime(), after.ctime())
    );
}
#[test]
fn planning_records_existing_and_broken_link_destinations_without_overwriting() {
    let (root, mut request) = fixture();
    let destination = root.path().join(&request.destination);
    for link in [false, true] {
        if link {
            symlink("missing-owned-target", &destination).unwrap();
        } else {
            fs::write(&destination, b"OWNED existing destination").unwrap();
        }
        request.expected_parent_version =
            super::super::observe(root.path(), Path::new("to"), false, None)
                .unwrap()
                .version;
        let plan = plan_worker(&request).unwrap();
        assert_eq!(
            plan.destination_before.unwrap().kind,
            if link {
                FileKind::Symlink
            } else {
                FileKind::File
            }
        );
        assert!(root.path().join(&request.path).exists());
        if !link {
            assert_eq!(
                fs::read(&destination).unwrap(),
                b"OWNED existing destination"
            );
        }
        fs::remove_file(&destination).unwrap();
    }
}
#[test]
fn planning_rejects_stale_source_or_destination_parent_and_source_links() {
    let (root, mut request) = fixture();
    let original = request.clone();
    request.expected_version = "stale".into();
    assert_eq!(
        plan_worker(&request).unwrap_err().code,
        FileErrorCode::Changed
    );
    request = original.clone();
    request.expected_parent_version = "stale".into();
    assert_eq!(
        plan_worker(&request).unwrap_err().code,
        FileErrorCode::Changed
    );
    symlink(
        root.path().join("from/owned"),
        root.path().join("from/link"),
    )
    .unwrap();
    request = original;
    request.path = "from/link".into();
    assert_eq!(
        plan_worker(&request).unwrap_err().code,
        FileErrorCode::SymlinkDisallowed
    );
    assert!(root.path().join("from/owned").exists());
    assert!(!root.path().join("to/renamed").exists());
}
#[test]
fn directory_package_and_hardlinked_sources_are_only_observed() {
    let (root, mut request) = fixture();
    fs::create_dir(root.path().join("Owned.app")).unwrap();
    fs::write(
        root.path().join("Owned.app/payload"),
        b"OWNED package content",
    )
    .unwrap();
    request.path = "Owned.app".into();
    request.expected_version = super::super::observe(root.path(), &request.path, false, None)
        .unwrap()
        .version;
    let plan = plan_worker(&request).unwrap();
    assert_eq!(plan.source.kind, FileKind::Directory);
    assert!(matches!(
        plan.source_resources.is_package,
        ResourceValue::Available { value: true }
    ));
    assert!(root.path().join("Owned.app/payload").exists());
    fs::hard_link(
        root.path().join("from/owned"),
        root.path().join("to/other-link"),
    )
    .unwrap();
    request.path = "from/owned".into();
    request.expected_version = super::super::observe(root.path(), &request.path, false, None)
        .unwrap()
        .version;
    request.expected_parent_version =
        super::super::observe(root.path(), Path::new("to"), false, None)
            .unwrap()
            .version;
    assert!(plan_worker(&request).is_ok());
    assert_eq!(
        fs::read(root.path().join("to/other-link")).unwrap(),
        b"OWNED\0relocation fixture"
    );
}
#[test]
fn directory_cannot_be_planned_into_itself_or_its_descendants() {
    let (root, mut request) = fixture();
    request.path = "from".into();
    request.destination = "from/nested".into();
    request.expected_version = super::super::observe(root.path(), &request.path, false, None)
        .unwrap()
        .version;
    request.expected_parent_version = request.expected_version.clone();
    assert_eq!(
        plan_worker(&request).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}
#[test]
fn same_name_rename_is_observed_as_noop_and_does_not_write() {
    let (root, mut request) = fixture();
    request.action = RelocationAction::Rename;
    request.destination = request.path.clone();
    request.expected_parent_version =
        super::super::observe(root.path(), Path::new("from"), false, None)
            .unwrap()
            .version;
    let plan = plan_worker(&request).unwrap();
    assert!(plan.no_op);
    assert_eq!(
        plan.destination_before.unwrap().identity,
        plan.source.identity
    );
}
