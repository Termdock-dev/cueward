use super::*;
use cueward_core::files::copy_tree::*;
use cueward_core::files::*;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::Path;

pub(super) fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("source/sub")).unwrap();
    fs::write(
        root.path().join("source/sub/臺灣\n<external>"),
        b"OWNED content\0",
    )
    .unwrap();
    fs::write(root.path().join("source/.hidden"), b"HIDDEN").unwrap();
    root
}
pub(super) fn request(root: &Path) -> CopyTreeRequest {
    CopyTreeRequest {
        root: root.into(),
        path: "source".into(),
        destination: "copy".into(),
        expected_version: super::super::observe(root, Path::new("source"), false, None)
            .unwrap()
            .version,
        expected_parent_version: super::super::observe(root, Path::new("."), false, None)
            .unwrap()
            .version,
        max_entries: 256,
        max_depth: 16,
        max_bytes: 67108864,
        include_packages: false,
    }
}
#[test]
fn copy_tree_sorted_hidden_unicode_and_empty_nodes_do_not_mutate_selected_data() {
    let root = fixture();
    fs::create_dir(root.path().join("source/empty")).unwrap();
    fs::hard_link(
        root.path().join("source/.hidden"),
        root.path().join("source/hardlink"),
    )
    .unwrap();
    let before = fs::metadata(root.path().join("source/.hidden")).unwrap();
    let plan = plan_worker(&request(root.path())).unwrap();
    assert!(!plan.has_blockers && !plan.execution_supported && plan.enumeration_complete);
    let names: Vec<_> = plan
        .entries
        .iter()
        .map(|e| e.relative_path.to_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            ".",
            ".hidden",
            "empty",
            "hardlink",
            "sub",
            "sub/臺灣\n<external>"
        ]
    );
    assert_eq!(
        plan.entries[5].destination,
        Path::new("copy/sub/臺灣\n<external>")
    );
    assert_eq!(
        plan.entries[0].source.version,
        plan.request.expected_version
    );
    assert_eq!(
        plan.observed_file_bytes,
        6 + 6 + b"OWNED content\0".len() as u64
    );
    assert_eq!(
        plan.entries[1].source.identity,
        plan.entries[3].source.identity
    );
    assert!(!root.path().join("copy").exists());
    let after = fs::metadata(root.path().join("source/.hidden")).unwrap();
    assert_eq!(MacFiles.stamp(&before), MacFiles.stamp(&after));
    assert_eq!(
        fs::read(root.path().join("source/sub/臺灣\n<external>")).unwrap(),
        b"OWNED content\0"
    );
}
#[test]
fn copy_tree_keeps_links_and_special_nodes_but_never_follows_them() {
    let root = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("PRIVATE"), b"not selected").unwrap();
    symlink(outside.path(), root.path().join("source/outside")).unwrap();
    symlink("missing", root.path().join("source/broken")).unwrap();
    assert!(
        std::process::Command::new("/usr/bin/mkfifo")
            .arg(root.path().join("source/pipe"))
            .status()
            .unwrap()
            .success()
    );
    let plan = plan_worker(&request(root.path())).unwrap();
    assert!(plan.has_blockers && plan.enumeration_complete);
    for name in ["outside", "broken", "pipe"] {
        let entry = plan
            .entries
            .iter()
            .find(|e| e.relative_path == Path::new(name))
            .unwrap();
        assert_eq!(
            entry.error.as_ref().unwrap().code,
            FileErrorCode::UnsupportedType
        );
        assert!(entry.resources.is_none());
    }
    assert!(!serde_json::to_string(&plan).unwrap().contains("PRIVATE"));
    assert!(!root.path().join("copy").exists());
}
#[test]
fn copy_tree_packages_are_blocked_without_inspecting_descendants() {
    let root = fixture();
    fs::create_dir(root.path().join("source/Owned.app")).unwrap();
    fs::write(
        root.path().join("source/Owned.app/PRIVATE"),
        b"OWNED package",
    )
    .unwrap();
    let plan = plan_worker(&request(root.path())).unwrap();
    assert!(plan.has_blockers && !plan.enumeration_complete);
    let entry = plan
        .entries
        .iter()
        .find(|e| e.relative_path == Path::new("Owned.app"))
        .unwrap();
    assert!(entry.error.is_some());
    assert!(
        !plan
            .entries
            .iter()
            .any(|e| e.relative_path.ends_with("PRIVATE"))
    );
}
#[test]
fn copy_tree_existing_targets_and_self_descendants_are_reported_not_created() {
    for kind in [0, 1, 2] {
        let root = fixture();
        match kind {
            0 => fs::write(root.path().join("copy"), b"OWNED conflict").unwrap(),
            1 => fs::create_dir(root.path().join("copy")).unwrap(),
            _ => symlink("missing", root.path().join("copy")).unwrap(),
        }
        let before = fs::symlink_metadata(root.path().join("copy"))
            .unwrap()
            .ino();
        let plan = plan_worker(&request(root.path())).unwrap();
        assert_eq!(plan.issues, [CopyTreeIssue::ExistingDestination]);
        assert!(plan.has_blockers && plan.destination_before.is_some());
        assert_eq!(
            fs::symlink_metadata(root.path().join("copy"))
                .unwrap()
                .ino(),
            before
        );
    }
    let root = fixture();
    let mut request = request(root.path());
    request.destination = "source/sub/copy".into();
    request.expected_parent_version =
        super::super::observe(root.path(), Path::new("source/sub"), false, None)
            .unwrap()
            .version;
    let plan = plan_worker(&request).unwrap();
    assert_eq!(plan.issues, [CopyTreeIssue::DestinationWithinSource]);
    assert!(!root.path().join("source/sub/copy").exists());
}
#[test]
fn copy_tree_limits_return_errors_without_a_partial_manifest() {
    let root = fixture();
    let original = request(root.path());
    for mode in [0, 1, 2] {
        let mut request = original.clone();
        match mode {
            0 => request.max_entries = 3,
            1 => request.max_depth = 1,
            _ => request.max_bytes = 1,
        }
        assert_eq!(
            plan_worker(&request).unwrap_err().code,
            FileErrorCode::ScanLimit
        );
    }
    for mode in 0..7 {
        let mut request = original.clone();
        match mode {
            0 => request.max_entries = 257,
            1 => request.max_depth = 33,
            2 => request.max_bytes = 0,
            3 => request.destination = "../outside".into(),
            4 => request.expected_version.clear(),
            5 => request.expected_parent_version.clear(),
            _ => request.root = "relative".into(),
        }
        assert_eq!(
            plan_worker(&request).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
    }
    assert!(!root.path().join("copy").exists());
}
#[test]
fn copy_tree_stale_revisions_and_symlink_parents_fail_closed() {
    let root = fixture();
    let mut stale = request(root.path());
    stale.expected_version = "stale".into();
    assert_eq!(
        plan_worker(&stale).unwrap_err().code,
        FileErrorCode::Changed
    );
    stale = request(root.path());
    stale.expected_parent_version = "stale".into();
    assert_eq!(
        plan_worker(&stale).unwrap_err().code,
        FileErrorCode::Changed
    );
    symlink("source/sub", root.path().join("parent-link")).unwrap();
    stale = request(root.path());
    stale.destination = "parent-link/copy".into();
    assert_eq!(
        plan_worker(&stale).unwrap_err().code,
        FileErrorCode::SymlinkDisallowed
    );
}
#[test]
fn copy_tree_timeout_stops_the_whole_readonly_worker() {
    let root = fixture();
    let executable = root.path().join("blocking");
    fs::write(&executable, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        run_plan(&executable, &request(root.path()), 100)
            .unwrap_err()
            .code,
        FileErrorCode::Timeout
    );
    assert!(!root.path().join("copy").exists());
}

#[test]
fn copy_tree_real_finder_alias_is_not_resolved_and_selected_metadata_is_preserved() {
    use objc2_foundation::{NSString, NSURL, NSURLBookmarkCreationOptions};
    let root = fixture();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("PRIVATE-target");
    fs::write(&target, b"not selected").unwrap();
    let alias = root.path().join("source/alias");
    let url = |path: &Path| {
        NSURL::fileURLWithPath_isDirectory(&NSString::from_str(path.to_str().unwrap()), false)
    };
    let data = url(&target)
        .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
            NSURLBookmarkCreationOptions::SuitableForBookmarkFile,
            None,
            None,
        )
        .unwrap();
    NSURL::writeBookmarkData_toURL_options_error(&data, &url(&alias), 0).unwrap();
    let bytes = fs::read(&alias).unwrap();
    let before = MacFiles.stamp(&alias.metadata().unwrap());
    let plan = plan_worker(&request(root.path())).unwrap();
    let entry = plan
        .entries
        .iter()
        .find(|e| e.relative_path == Path::new("alias"))
        .unwrap();
    assert!(plan.has_blockers && plan.enumeration_complete);
    assert_eq!(
        entry.error.as_ref().unwrap().code,
        FileErrorCode::UnsupportedType
    );
    assert_eq!(
        entry.resources.as_ref().unwrap().is_alias_file,
        ResourceValue::Available { value: true }
    );
    assert_eq!(MacFiles.stamp(&alias.metadata().unwrap()), before);
    assert_eq!(fs::read(alias).unwrap(), bytes);
    assert!(
        !plan
            .entries
            .iter()
            .any(|e| e.source.path.contains("PRIVATE-target"))
    );
}

#[test]
fn copy_tree_special_metadata_blocks_files_without_dropping_other_nodes() {
    let root = fixture();
    fs::set_permissions(
        root.path().join("source/.hidden"),
        fs::Permissions::from_mode(0o4644),
    )
    .unwrap();
    let plan = plan_worker(&request(root.path())).unwrap();
    assert!(plan.has_blockers && plan.enumeration_complete);
    assert_eq!(
        plan.entries[1].error.as_ref().unwrap().code,
        FileErrorCode::UnsupportedType
    );
    assert_eq!(plan.entries.len(), 4);
    assert!(!root.path().join("copy").exists());
}
