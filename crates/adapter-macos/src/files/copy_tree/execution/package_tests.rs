use super::tests::{direct, fixture, owned, perform, request};
use super::*;
use cueward_core::files::mutation::MutationStatus;
use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};
fn package_fixture() -> tests::Owned {
    let root = fixture();
    let mut selected = request(root.path());
    fs::rename(root.path().join("source"), root.path().join("Owned.app")).unwrap();
    fs::create_dir(root.path().join("Owned.app/Nested.app")).unwrap();
    fs::write(
        root.path().join("Owned.app/Nested.app/Info.plist"),
        b"OWNED nested package",
    )
    .unwrap();
    selected.path = "Owned.app".into();
    selected.destination = "Owned-copy.app".into();
    selected.include_packages = true;
    refresh(root.path(), &mut selected);
    owned(root, selected)
}
fn refresh(root: &Path, selected: &mut CopyTreeRequest) {
    selected.expected_version = crate::files::observe(root, &selected.path, false, None)
        .unwrap()
        .version;
    selected.expected_parent_version = crate::files::observe(root, Path::new("."), false, None)
        .unwrap()
        .version;
}
#[test]
fn package_execute_uses_verified_tree_copy_with_independent_nodes_and_persistent_opt_in() {
    let mut value = package_fixture();
    let source = value.root.path().join("Owned.app");
    assert!(
        std::process::Command::new("/usr/bin/xattr")
            .args(["-w", "com.cueward.owned-package", "OWNED metadata"])
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    refresh(value.root.path(), &mut value.receipt.request);
    save(&value.receipt).unwrap();
    perform(&mut value);
    assert_eq!(
        value.receipt.status,
        MutationStatus::Completed,
        "{:?}",
        value.receipt.error
    );
    assert!(value.receipt.completion_verified && value.receipt.staging_verified);
    assert!(
        value
            .receipt
            .nodes
            .iter()
            .any(|n| n.relative_path == Path::new("Nested.app/Info.plist"))
    );
    for node in &value.receipt.nodes {
        let before = source.join(&node.relative_path);
        let after = value
            .root
            .path()
            .join("Owned-copy.app")
            .join(&node.relative_path);
        assert_ne!(
            fs::metadata(&before).unwrap().ino(),
            fs::metadata(&after).unwrap().ino()
        );
        assert_eq!(
            fs::metadata(&before).unwrap().modified().unwrap(),
            fs::metadata(&after).unwrap().modified().unwrap()
        );
        assert_eq!(
            fs::metadata(&before).unwrap().permissions(),
            fs::metadata(&after).unwrap().permissions()
        );
        if node.kind == FileKind::File {
            assert_eq!(fs::read(&before).unwrap(), fs::read(after).unwrap());
            assert!(node.verification.is_some());
        }
    }
    let attribute = std::process::Command::new("/usr/bin/xattr")
        .args(["-p", "com.cueward.owned-package"])
        .arg(value.root.path().join("Owned-copy.app"))
        .output()
        .unwrap();
    assert!(attribute.status.success());
    assert_eq!(
        String::from_utf8(attribute.stdout).unwrap().trim_end(),
        "OWNED metadata"
    );
    let saved = read_receipt(&value.receipt.operation_id).unwrap();
    assert!(saved.request.include_packages);
    assert_eq!(json(&saved).unwrap(), json(&value.receipt).unwrap());
}
#[test]
fn package_execute_copies_link_objects_and_refuses_missing_opt_in_limits_or_conflicts() {
    for mode in 0..5 {
        let mut value = package_fixture();
        match mode {
            0 => value.receipt.request.include_packages = false,
            1 => symlink("missing", value.root.path().join("Owned.app/link")).unwrap(),
            2 => value.receipt.request.max_entries = 1,
            3 => fs::write(value.root.path().join("Owned-copy.app"), b"OWNED conflict").unwrap(),
            4 => value.receipt.request.max_bytes = 1,
            _ => unreachable!(),
        }
        if mode == 1 || mode == 3 {
            refresh(value.root.path(), &mut value.receipt.request);
        }
        save(&value.receipt).unwrap();
        perform(&mut value);
        if mode == 1 {
            assert_eq!(
                value.receipt.status,
                MutationStatus::Completed,
                "{:?}",
                value.receipt.error
            );
            assert_eq!(
                fs::read_link(value.root.path().join("Owned-copy.app/link")).unwrap(),
                Path::new("missing")
            );
            continue;
        }
        assert_eq!(
            value.receipt.status,
            MutationStatus::NotStarted,
            "mode={mode} {:?}",
            value.receipt.error
        );
        assert!(value.receipt.staging_path.is_none() && !value.receipt.mutation_attempted);
        assert!(
            value
                .root
                .path()
                .join("Owned.app/sub/臺灣\n<external>")
                .is_file()
        );
    }
}
#[test]
fn package_copy_stops_on_nested_content_edit_after_staging_without_touching_source_or_publishing() {
    let mut value = package_fixture();
    let source = value.root.path().join("Owned.app/Nested.app/Info.plist");
    direct(&MacFiles, &mut value, |r| {
        if r.stage == TreeStage::Staged {
            fs::write(&source, b"OWNED external change").unwrap();
        }
        Ok(())
    });
    assert_eq!(
        value.receipt.status,
        MutationStatus::NotStarted,
        "{:?}",
        value.receipt.error
    );
    assert!(!value.root.path().join("Owned-copy.app").exists());
    assert_eq!(fs::read(&source).unwrap(), b"OWNED external change");
    assert_eq!(
        fs::read(
            Path::new(value.receipt.staging_path.as_ref().unwrap()).join("Nested.app/Info.plist")
        )
        .unwrap(),
        b"OWNED nested package"
    );
}
