use super::tests::{fixture, request};
use super::*;
use cueward_core::files::{FileErrorCode, ResourceValue};
use std::fs;
use std::os::unix::fs::symlink;
#[test]
fn package_inclusion_plans_selected_and_nested_known_packages_only_when_opted_in() {
    let root = fixture();
    fs::create_dir_all(root.path().join("source/Owned.app/Nested.app")).unwrap();
    fs::write(
        root.path()
            .join("source/Owned.app/Nested.app/臺灣\n<external>"),
        b"OWNED package bytes",
    )
    .unwrap();
    for selected_root in [false, true] {
        let mut selected = request(root.path());
        if selected_root {
            selected.path = "source/Owned.app".into();
            selected.destination = "Owned-copy.app".into();
            selected.expected_version =
                crate::files::observe(root.path(), &selected.path, false, None)
                    .unwrap()
                    .version;
        }
        let blocked = plan_worker(&selected).unwrap();
        assert!(blocked.has_blockers && !blocked.enumeration_complete);
        assert!(
            !blocked.entries.iter().any(
                |e| e.source.path.contains("Nested.app") && e.source.name == "臺灣\n<external>"
            )
        );
        selected.include_packages = true;
        let plan = plan_worker(&selected).unwrap();
        assert!(!plan.has_blockers && plan.enumeration_complete && !plan.execution_supported);
        assert!(
            plan.entries.iter().any(
                |e| e.source.path.contains("Nested.app") && e.source.name == "臺灣\n<external>"
            )
        );
        let packages: Vec<_> = plan
            .entries
            .iter()
            .filter(|e| {
                e.resources
                    .as_ref()
                    .is_some_and(|r| r.is_package == (ResourceValue::Available { value: true }))
            })
            .collect();
        assert_eq!(packages.len(), 2);
        assert!(packages.iter().all(|e| e.error.is_none()));
        assert!(!root.path().join(&selected.destination).exists());
    }
}
#[test]
fn package_inclusion_does_not_follow_links_and_still_enforces_tree_budgets() {
    let root = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("PRIVATE"), b"not selected").unwrap();
    fs::create_dir(root.path().join("source/Owned.app")).unwrap();
    symlink(outside.path(), root.path().join("source/Owned.app/link")).unwrap();
    let mut selected = request(root.path());
    selected.include_packages = true;
    let plan = plan_worker(&selected).unwrap();
    assert!(plan.has_blockers);
    assert!(!serde_json::to_string(&plan).unwrap().contains("PRIVATE"));
    let link = plan
        .entries
        .iter()
        .find(|e| e.relative_path == Path::new("Owned.app/link"))
        .unwrap();
    assert_eq!(
        link.error.as_ref().unwrap().code,
        FileErrorCode::UnsupportedType
    );
    for mode in 0..3 {
        let mut limited = selected.clone();
        match mode {
            0 => limited.max_entries = 1,
            1 => limited.max_depth = 1,
            2 => limited.max_bytes = 1,
            _ => unreachable!(),
        }
        assert_eq!(
            plan_worker(&limited).unwrap_err().code,
            FileErrorCode::ScanLimit
        );
    }
    assert!(!root.path().join("copy").exists());
}
#[test]
fn package_request_schema_defaults_old_requests_to_safe_skip_and_rejects_non_boolean_inclusion() {
    let root = fixture();
    let selected = request(root.path());
    let mut json = serde_json::to_value(&selected).unwrap();
    assert!(json.get("include_packages").is_none());
    let old: CopyTreeRequest = serde_json::from_value(json.clone()).unwrap();
    assert!(!old.include_packages);
    json["include_packages"] = serde_json::json!(true);
    let included: CopyTreeRequest = serde_json::from_value(json.clone()).unwrap();
    assert!(included.include_packages);
    assert_eq!(
        serde_json::to_value(included).unwrap()["include_packages"],
        true
    );
    for value in [
        serde_json::json!("true"),
        serde_json::Value::Null,
        serde_json::json!(1),
    ] {
        json["include_packages"] = value;
        assert!(serde_json::from_value::<CopyTreeRequest>(json.clone()).is_err());
    }
}
