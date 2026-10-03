//! Per-entry link-object permission reuses independent batch planning and child supervision.
use super::*;
use crate::files::batch_rename;
use cueward_core::files::{FileKind, FilePlatform, ResourceValue};

fn links() -> tempfile::TempDir {
    let root = fixture();
    symlink("missing\n<external>", root.path().join("link")).unwrap();
    fs::create_dir(root.path().join("nested")).unwrap();
    symlink("../a", root.path().join("nested/link")).unwrap();
    root
}
fn include(mut request: BatchRenameRequest, indices: &[usize]) -> BatchRenameRequest {
    for &index in indices {
        request.entries[index].link_itself = true;
    }
    request
}
#[test]
fn batch_link_plans_keep_default_refusal_and_per_entry_permission_without_target_metadata() {
    let root = links();
    let request = request(
        root.path(),
        &[
            ("link", "新\n<external>"),
            ("a", "ordinary"),
            ("nested/link", "new"),
        ],
    );
    let blocked = batch_rename::plan_worker(&request).unwrap();
    assert!(
        blocked.has_conflicts
            && blocked.items[0].error.is_some()
            && blocked.items[2].error.is_some()
    );
    let selected = include(request, &[0, 2]);
    let plan = batch_rename::plan_worker(&selected).unwrap();
    assert!(!plan.has_conflicts && !plan.execution_supported);
    for (index, item) in plan.items.iter().enumerate() {
        let proposal = item.proposal.as_ref().unwrap();
        assert_eq!(proposal.request.link_itself, index != 1);
        if index != 1 {
            assert_eq!(proposal.source.kind, FileKind::Symlink);
            assert_eq!(
                proposal.source_resources.is_alias_file,
                ResourceValue::NotApplicable
            );
        }
        assert!(fs::symlink_metadata(root.path().join(&proposal.request.destination)).is_err());
    }
    assert_eq!(
        fs::read_link(root.path().join("link")).unwrap(),
        Path::new("missing\n<external>")
    );
}
#[test]
fn batch_link_execution_verifies_mixed_parents_noops_inodes_references_and_saved_receipts() {
    let root = links();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("PRIVATE");
    fs::write(&target, b"OWNED outside").unwrap();
    symlink(&target, root.path().join("outside")).unwrap();
    let before = MacFiles.stamp(&fs::metadata(&target).unwrap());
    let request = include(
        request(
            root.path(),
            &[
                ("link", "新\n<external>"),
                ("a", "ordinary"),
                ("nested/link", "new"),
                ("outside", "outside"),
            ],
        ),
        &[0, 2, 3],
    );
    let inodes: Vec<_> = request
        .entries
        .iter()
        .map(|e| {
            fs::symlink_metadata(root.path().join(&e.path))
                .unwrap()
                .ino()
        })
        .collect();
    let receipt = perform(&MacFiles, &request);
    assert_eq!(
        receipt.status,
        RelocationStatus::Completed,
        "{:?}",
        receipt.error
    );
    assert!(receipt.completion_verified);
    for (index, item) in receipt.items.iter().enumerate() {
        let child = relocation::read_receipt(item.operation_id.as_ref().unwrap()).unwrap();
        assert_eq!(child.request.link_itself, index != 1);
        assert_eq!(
            child.destination_after.as_ref().unwrap().identity,
            child.before.as_ref().unwrap().source.identity
        );
        assert_eq!(
            fs::symlink_metadata(&child.destination_after.as_ref().unwrap().path)
                .unwrap()
                .ino(),
            inodes[index]
        );
        assert_eq!(
            child.destination_after.as_ref().unwrap().link_target,
            child.before.as_ref().unwrap().source.link_target
        );
        if index == 3 {
            assert_eq!(child.renamed, Some(false));
        }
        if index == 1 {
            assert!(
                !serde_json::to_value(&child).unwrap()["request"]
                    .as_object()
                    .unwrap()
                    .contains_key("link_itself")
            );
        }
    }
    let second = relocation::read_receipt(receipt.items[1].operation_id.as_ref().unwrap()).unwrap();
    assert_ne!(
        second.request.expected_parent_version,
        request.entries[1].expected_parent_version
    );
    assert_eq!(MacFiles.stamp(&fs::metadata(&target).unwrap()), before);
    assert_eq!(fs::read(&target).unwrap(), b"OWNED outside");
    assert_eq!(
        serde_json::to_value(read_receipt(&receipt.operation_id).unwrap()).unwrap(),
        serde_json::to_value(&receipt).unwrap()
    );
    assert_eq!(
        execute_worker(&BatchExecutionWorkerRequest {
            operation_id: receipt.operation_id.clone()
        })
        .unwrap_err()
        .code,
        FileErrorCode::Conflict
    );
    cleanup(&receipt);
}
#[test]
fn batch_link_opt_in_cannot_bypass_nonlink_errors_conflicts_chains_or_stale_versions() {
    for mode in 0..6 {
        let root = links();
        let mut r = include(
            request(root.path(), &[("a", "ordinary"), ("link", "renamed")]),
            &[1],
        );
        match mode {
            0 => r.entries[1].link_itself = false,
            1 => r.entries[0].link_itself = true,
            2 => r.entries[1].name = "a".into(),
            3 => r.entries[1].expected_version = "stale".into(),
            4 => r.entries[1].name = "ordinary".into(),
            5 => {
                r.entries[0].name = "link".into();
                r.entries[1].name = "a".into();
            }
            _ => unreachable!(),
        }
        let receipt = perform(&MacFiles, &r);
        assert_eq!(
            receipt.status,
            RelocationStatus::NotStarted,
            "mode={mode} {:?}",
            receipt.error
        );
        assert!(
            receipt
                .items
                .iter()
                .all(|i| i.operation_id.is_none() && !i.mutation_attempted)
        );
        assert_eq!(fs::read(root.path().join("a")).unwrap(), b"OWNED a\0");
        assert_eq!(
            fs::read_link(root.path().join("link")).unwrap(),
            Path::new("missing\n<external>")
        );
        cleanup(&receipt);
    }
}
#[test]
fn batch_link_entry_schema_retains_legacy_omission_strict_unknown_fields_and_boolean_permission() {
    let root = links();
    let r = request(root.path(), &[("link", "new")]);
    let mut value = serde_json::to_value(&r).unwrap();
    assert!(value["entries"][0].get("link_itself").is_none());
    assert!(
        !serde_json::from_value::<BatchRenameRequest>(value.clone())
            .unwrap()
            .entries[0]
            .link_itself
    );
    value["entries"][0]["link_itself"] = serde_json::json!(true);
    assert!(
        serde_json::from_value::<BatchRenameRequest>(value.clone())
            .unwrap()
            .entries[0]
            .link_itself
    );
    for v in [
        serde_json::json!("true"),
        serde_json::json!(null),
        serde_json::json!(1),
    ] {
        value["entries"][0]["link_itself"] = v;
        assert!(serde_json::from_value::<BatchRenameRequest>(value.clone()).is_err());
    }
    value["entries"][0]["link_itself"] = serde_json::json!(true);
    value["entries"][0]["overwrite"] = serde_json::json!(true);
    assert!(serde_json::from_value::<BatchRenameRequest>(value).is_err());
}
