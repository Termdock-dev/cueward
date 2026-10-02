use super::*;
use cueward_core::files::{FileErrorCode, FilePlatform};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("nested")).unwrap();
    for name in ["a", "b", "nested/c"] {
        fs::write(root.path().join(name), format!("OWNED {name}\0")).unwrap();
    }
    root
}
pub(super) fn request(root: &Path, proposals: &[(&str, &str)]) -> BatchRenameRequest {
    BatchRenameRequest {
        root: root.into(),
        entries: proposals
            .iter()
            .map(|(path, name)| {
                let parent = Path::new(path)
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                BatchRenameEntry {
                    path: path.into(),
                    name: (*name).into(),
                    expected_version: super::super::observe(root, Path::new(path), false, None)
                        .unwrap()
                        .version,
                    expected_parent_version: super::super::observe(root, parent, false, None)
                        .unwrap()
                        .version,
                }
            })
            .collect(),
    }
}
fn has(plan: &BatchRenamePlan, code: BatchRenameIssueCode, indices: &[usize]) -> bool {
    plan.issues
        .iter()
        .any(|issue| issue.code == code && issue.entries == indices)
}
#[test]
fn batch_rename_observes_order_exact_names_and_versions_without_writes() {
    let root = fixture();
    let request = request(
        root.path(),
        &[("a", "臺灣\n<external>"), ("nested/c", "c-copy")],
    );
    let before = fs::metadata(root.path().join("a")).unwrap();
    let plan = plan_worker(&request).unwrap();
    assert!(!plan.has_conflicts && !plan.execution_supported);
    assert!(plan.issues.is_empty());
    for (index, item) in plan.items.iter().enumerate() {
        assert_eq!(item.index, index);
        assert!(item.error.is_none());
        let proposal = item.proposal.as_ref().unwrap();
        assert_eq!(
            proposal.source.version,
            request.entries[index].expected_version
        );
        assert_eq!(
            proposal
                .request
                .destination
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
            request.entries[index].name
        );
        assert!(!root.path().join(&proposal.request.destination).exists());
        assert!(root.path().join(&proposal.request.path).exists());
    }
    let after = fs::metadata(root.path().join("a")).unwrap();
    assert_eq!(
        (before.ino(), before.mode(), before.mtime(), before.ctime()),
        (after.ino(), after.mode(), after.mtime(), after.ctime())
    );
    assert_eq!(fs::read(root.path().join("a")).unwrap(), b"OWNED a\0");
}
#[test]
fn batch_rename_keeps_noops_and_existing_file_directory_broken_link_conflicts() {
    let root = fixture();
    fs::create_dir(root.path().join("existing-dir")).unwrap();
    symlink("missing-owned", root.path().join("broken")).unwrap();
    for name in ["b", "existing-dir", "broken"] {
        let plan = plan_worker(&request(root.path(), &[("a", name)])).unwrap();
        assert!(plan.has_conflicts);
        assert!(has(&plan, BatchRenameIssueCode::ExistingDestination, &[0]));
        assert!(
            plan.items[0]
                .proposal
                .as_ref()
                .unwrap()
                .destination_before
                .is_some()
        );
    }
    let plan = plan_worker(&request(root.path(), &[("a", "a")])).unwrap();
    assert!(!plan.has_conflicts && plan.items[0].proposal.as_ref().unwrap().no_op);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 5);
}
#[test]
fn batch_rename_reports_duplicate_sources_destinations_and_shared_hardlink_identity() {
    let root = fixture();
    fs::hard_link(root.path().join("a"), root.path().join("link-a")).unwrap();
    for (entries, code) in [
        (
            vec![("a", "new"), ("a", "other")],
            BatchRenameIssueCode::DuplicateSource,
        ),
        (
            vec![("a", "new"), ("b", "new")],
            BatchRenameIssueCode::DuplicateDestination,
        ),
        (
            vec![("a", "new"), ("link-a", "other")],
            BatchRenameIssueCode::SharedSourceIdentity,
        ),
    ] {
        let plan = plan_worker(&request(root.path(), &entries)).unwrap();
        assert!(plan.has_conflicts && has(&plan, code, &[0, 1]));
    }
}
#[test]
fn batch_rename_conservatively_compares_case_and_canonical_unicode_only_in_same_parent() {
    let root = fixture();
    for (left, right) in [("Report", "report"), ("café", "café"), ("CAFÉ", "café")] {
        let plan = plan_worker(&request(root.path(), &[("a", left), ("b", right)])).unwrap();
        assert!(has(
            &plan,
            BatchRenameIssueCode::PotentialDestinationCollision,
            &[0, 1]
        ));
        assert_eq!(plan.request.entries[0].name, left);
        assert_eq!(plan.request.entries[1].name, right);
    }
    let plan = plan_worker(&request(
        root.path(),
        &[("a", "Report"), ("nested/c", "report")],
    ))
    .unwrap();
    assert!(!plan.has_conflicts);
    assert!(!MacFiles.names_may_collide("a", "á"));
}
#[test]
fn batch_rename_reports_swaps_chains_and_nested_directory_sources_without_ordering() {
    let root = fixture();
    for entries in [vec![("a", "b"), ("b", "a")], vec![("a", "b"), ("b", "new")]] {
        let plan = plan_worker(&request(root.path(), &entries)).unwrap();
        assert!(has(
            &plan,
            BatchRenameIssueCode::SourceDestinationDependency,
            &[0, 1]
        ));
        assert!(has(&plan, BatchRenameIssueCode::ExistingDestination, &[0]));
    }
    let plan = plan_worker(&request(
        root.path(),
        &[("nested", "renamed"), ("nested/c", "new")],
    ))
    .unwrap();
    assert!(has(&plan, BatchRenameIssueCode::NestedSource, &[0, 1]));
    assert!(root.path().join("nested/c").exists());
    assert!(!root.path().join("renamed").exists());
}
#[test]
fn batch_rename_item_errors_preserve_indices_and_do_not_hide_valid_proposals() {
    let root = fixture();
    symlink("a", root.path().join("source-link")).unwrap();
    let mut request = request(
        root.path(),
        &[
            ("a", "new"),
            ("source-link", "other"),
            ("b", "../bad"),
            ("nested/c", "new"),
        ],
    );
    request.entries[3].expected_version = "stale".into();
    let plan = plan_worker(&request).unwrap();
    assert!(plan.has_conflicts && !plan.execution_supported);
    assert_eq!(plan.items.len(), 4);
    assert!(plan.items[0].proposal.is_some());
    for (index, code) in [
        (1, FileErrorCode::SymlinkDisallowed),
        (2, FileErrorCode::InvalidOptions),
        (3, FileErrorCode::Changed),
    ] {
        assert_eq!(plan.items[index].index, index);
        assert_eq!(plan.items[index].error.as_ref().unwrap().code, code);
        assert!(plan.items[index].proposal.is_none());
    }
    assert!(root.path().join("a").exists());
    assert!(!root.path().join("new").exists());
}
#[test]
fn batch_rename_rejects_invalid_scope_count_and_worker_payload_budgets() {
    let root = fixture();
    let original = request(root.path(), &[("a", "new")]);
    for count in [0, 65] {
        let mut request = original.clone();
        request.entries = vec![request.entries[0].clone(); count];
        assert_eq!(
            plan_worker(&request).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
    }
    let mut request = original.clone();
    request.entries[0].name = "x".repeat(16384);
    assert_eq!(
        plan_worker(&request).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
    request = original;
    request.root = "relative".into();
    assert_eq!(
        plan_worker(&request).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}
#[test]
fn batch_rename_timeout_stops_readonly_worker_without_names_changing() {
    let root = fixture();
    let executable = root.path().join("blocking-worker");
    fs::write(&executable, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let request = request(root.path(), &[("a", "new")]);
    assert_eq!(
        run_plan(&executable, &request, 100).unwrap_err().code,
        FileErrorCode::Timeout
    );
    assert!(root.path().join("a").exists());
    assert!(!root.path().join("new").exists());
}

#[path = "batch_rename_safety_tests.rs"]
mod safety;

#[test]
fn batch_rename_core_accepts_count_boundary_and_retains_all_failed_items() {
    let root = fixture();
    let mut request = request(root.path(), &[("a", "new")]);
    request.entries[0].path = "missing".into();
    request.entries = vec![request.entries[0].clone(); batch::MAX_BATCH_RENAMES];
    let plan = batch::plan(&MacFiles, &request).unwrap();
    assert_eq!(plan.items.len(), 64);
    assert!(plan.has_conflicts && !plan.execution_supported);
    assert_eq!(plan.items[63].index, 63);
    assert!(
        plan.items
            .iter()
            .all(|item| item.error.as_ref().unwrap().code == FileErrorCode::NotFound)
    );
    request.entries.push(request.entries[0].clone());
    assert_eq!(
        batch::plan(&MacFiles, &request).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}

#[test]
fn batch_rename_packages_and_directories_are_observed_without_inspecting_contents() {
    let root = fixture();
    fs::create_dir(root.path().join("Owned.app")).unwrap();
    fs::write(
        root.path().join("Owned.app/payload"),
        b"OWNED package payload",
    )
    .unwrap();
    let plan = plan_worker(&request(
        root.path(),
        &[("Owned.app", "New.app"), ("nested", "new-nested")],
    ))
    .unwrap();
    assert!(!plan.has_conflicts);
    assert_eq!(
        plan.items[0].proposal.as_ref().unwrap().source.kind,
        cueward_core::files::FileKind::Directory
    );
    assert!(root.path().join("Owned.app/payload").exists());
    assert!(!root.path().join("New.app").exists());
}
