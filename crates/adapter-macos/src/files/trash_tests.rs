use super::*;
use crate::files::observe;
use cueward_core::files::trash::{TrashTargetKind as Kind, TrashWarning as Warning};
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt, symlink};

pub(super) fn request(root: &Path, path: &str) -> TrashPlanRequest {
    TrashPlanRequest {
        root: root.into(),
        path: path.into(),
        expected_version: observe(root, Path::new(path), false, None).unwrap().version,
    }
}
#[test]
fn trash_plan_preserves_regular_payload_metadata_hardlinks_and_exact_external_names() {
    let root = tempfile::tempdir().unwrap();
    let name = "臺灣\n<external>";
    fs::write(root.path().join(name), b"OWNED\0</external>").unwrap();
    fs::hard_link(root.path().join(name), root.path().join("sibling")).unwrap();
    fs::set_permissions(root.path().join(name), fs::Permissions::from_mode(0o444)).unwrap();
    let before = fs::metadata(root.path().join(name)).unwrap();
    let selected = request(root.path(), name);
    let result = plan_worker(&selected).unwrap();
    assert_eq!(result.target_kind, Kind::RegularFile);
    assert!(
        result.requires_confirmation && !result.execution_supported && !result.recovery_supported
    );
    assert!(!result.descendants_inspected && !result.link_targets_inspected);
    assert!(result.trash_destination.is_none());
    assert!(result.warnings.contains(&Warning::SourceReadonlyMode));
    assert_eq!(result.source.name, name);
    assert_eq!(result.source.version, selected.expected_version);
    assert_eq!(
        fs::read(root.path().join(name)).unwrap(),
        b"OWNED\0</external>"
    );
    let after = fs::metadata(root.path().join(name)).unwrap();
    assert_eq!(
        (
            before.ino(),
            before.nlink(),
            before.mode(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec()
        ),
        (
            after.ino(),
            after.nlink(),
            after.mode(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec()
        )
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    let value = serde_json::to_value(result).unwrap();
    assert!(value.get("operation_id").is_none() && value.get("receipt_path").is_none());
}
#[test]
fn trash_plan_directory_and_package_are_whole_entries_without_descendant_inspection() {
    let root = tempfile::tempdir().unwrap();
    for (name, kind) in [
        ("ordinary", Kind::Directory),
        ("Owned.app", Kind::PackageDirectory),
    ] {
        fs::create_dir_all(root.path().join(name).join("hidden/sub")).unwrap();
        fs::write(
            root.path().join(name).join("hidden/.data"),
            b"OWNED descendant",
        )
        .unwrap();
        let result = plan_worker(&request(root.path(), name)).unwrap();
        assert_eq!(result.target_kind, kind);
        assert!(result.warnings.contains(&Warning::WholeDirectoryEntry));
        assert!(!result.descendants_inspected);
        assert_eq!(
            fs::read(root.path().join(name).join("hidden/.data")).unwrap(),
            b"OWNED descendant"
        );
    }
}
#[test]
fn trash_plan_leaf_links_describe_link_itself_without_querying_outside_or_broken_targets() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("target"), b"OWNED outside").unwrap();
    symlink(
        outside.path().join("target"),
        root.path().join("outside-link"),
    )
    .unwrap();
    symlink("missing", root.path().join("broken")).unwrap();
    for name in ["outside-link", "broken"] {
        let selected = request(root.path(), name);
        let result = plan_worker(&selected).unwrap();
        assert_eq!(result.target_kind, Kind::Symlink);
        assert_eq!(result.warnings, vec![Warning::SymlinkItself]);
        assert!(result.source.resolved_path.is_none());
        assert!(!result.link_targets_inspected);
        assert_eq!(
            result.resources,
            cueward_core::files::ResourceMetadata {
                content_type: cueward_core::files::ResourceValue::NotApplicable,
                finder_tags: cueward_core::files::ResourceValue::NotApplicable,
                is_package: cueward_core::files::ResourceValue::NotApplicable,
                is_alias_file: cueward_core::files::ResourceValue::NotApplicable,
            }
        );
        assert_eq!(
            request(root.path(), name).expected_version,
            selected.expected_version
        );
    }
    assert_eq!(
        fs::read(outside.path().join("target")).unwrap(),
        b"OWNED outside"
    );
}
#[test]
fn trash_plan_special_entry_is_observed_without_fifo_payload_open() {
    let root = tempfile::tempdir().unwrap();
    let fifo = root.path().join("fifo");
    let path = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: owned new fixture path represented by a valid NUL-terminated string.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    let result = plan_worker(&request(root.path(), "fifo")).unwrap();
    assert_eq!(result.target_kind, Kind::SpecialEntry);
    assert!(result.warnings.contains(&Warning::SpecialEntry));
    assert!(!result.execution_supported);
    assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
}
#[test]
fn trash_plan_invalid_stale_and_missing_selection_return_errors_without_changes() {
    use cueward_core::files::FileErrorCode as Code;
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("owned"), b"OWNED").unwrap();
    let selected = request(root.path(), "owned");
    for path in [
        "",
        ".",
        "..",
        "../outside",
        "/absolute",
        "NUL\0path",
        "owned/",
    ] {
        let mut invalid = selected.clone();
        invalid.path = path.into();
        assert_eq!(
            plan_worker(&invalid).unwrap_err().code,
            Code::InvalidOptions
        );
    }
    let mut invalid = selected.clone();
    invalid.expected_version.clear();
    assert_eq!(
        plan_worker(&invalid).unwrap_err().code,
        Code::InvalidOptions
    );
    invalid.expected_version = "stale".into();
    assert_eq!(plan_worker(&invalid).unwrap_err().code, Code::Changed);
    invalid = selected;
    invalid.path = "missing".into();
    assert_eq!(plan_worker(&invalid).unwrap_err().code, Code::NotFound);
    assert_eq!(fs::read(root.path().join("owned")).unwrap(), b"OWNED");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
#[test]
fn trash_plan_refuses_ancestor_links_but_accepts_a_stable_selected_root_alias() {
    use cueward_core::files::FileErrorCode as Code;
    let root = tempfile::tempdir().unwrap();
    let aliases = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sub")).unwrap();
    fs::write(root.path().join("sub/owned"), b"OWNED").unwrap();
    symlink(root.path(), aliases.path().join("root")).unwrap();
    let alias = aliases.path().join("root");
    let result = plan_worker(&request(&alias, "sub/owned")).unwrap();
    assert_eq!(result.request.root, alias);
    assert_eq!(
        result.root.path,
        root.path().canonicalize().unwrap().to_str().unwrap()
    );
    symlink(root.path().join("sub"), root.path().join("link")).unwrap();
    let selected = TrashPlanRequest {
        root: root.path().into(),
        path: "link/owned".into(),
        expected_version: "unused".into(),
    };
    assert_eq!(
        plan_worker(&selected).unwrap_err().code,
        Code::SymlinkDisallowed
    );
}
