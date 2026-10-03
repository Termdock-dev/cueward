use super::*;
use cueward_core::files::mutation::*;
use std::ffi::OsStr;
use std::fs::{self, File, Metadata};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

pub(in crate::files::trash) struct Owned {
    pub root: tempfile::TempDir,
    pub trash: tempfile::TempDir,
    pub receipt: TrashReceipt,
}
impl Drop for Owned {
    fn drop(&mut self) {
        let directory = Path::new(&self.receipt.receipt_path).parent().unwrap();
        assert_eq!(
            directory.file_name().unwrap().to_str().unwrap(),
            self.receipt.operation_id
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
pub(in crate::files::trash) fn request(root: &Path) -> TrashRequest {
    let observed = |path| {
        crate::files::observe(root, Path::new(path), false, None)
            .unwrap()
            .version
    };
    TrashRequest {
        root: root.into(),
        path: "source".into(),
        expected_version: observed("source"),
        expected_parent_version: observed("."),
        confirm: true,
        max_bytes: 67108864,
    }
}
pub(in crate::files::trash) fn fixture() -> Owned {
    let root = tempfile::tempdir().unwrap();
    let trash = tempfile::tempdir().unwrap();
    fs::set_permissions(trash.path(), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(root.path().join("source"), b"OWNED bytes\0</external>").unwrap();
    let selected = request(root.path());
    let receipt = STORE
        .create(|id, path| TrashReceipt::new(id, path, selected))
        .unwrap();
    Owned {
        root,
        trash,
        receipt,
    }
}
pub(in crate::files::trash) struct Controlled {
    pub trash: PathBuf,
    pub race: Option<Box<dyn Fn()>>,
    pub uncertain: bool,
    pub pause: Option<PathBuf>,
    pub unknown_alias: bool,
    pub unknown_data: bool,
    pub native_mode: u8,
    pub native_pause: u8,
    pub storage_inside: bool,
}
pub(in crate::files::trash) fn platform(value: &Owned) -> Controlled {
    Controlled {
        trash: value.trash.path().canonicalize().unwrap(),
        race: None,
        uncertain: false,
        pause: None,
        unknown_alias: false,
        unknown_data: false,
        native_mode: 0,
        native_pause: 0,
        storage_inside: false,
    }
}
impl FilePlatform for Controlled {
    fn stamp(&self, metadata: &Metadata) -> FileStamp {
        let mut stamp = MacFiles.stamp(metadata);
        if self.unknown_data {
            stamp.data_state = DataState::Unknown;
        }
        stamp
    }
    fn open_regular(&self, path: &Path) -> std::io::Result<File> {
        MacFiles.open_regular(path)
    }
    fn resource_metadata(
        &self,
        path: &Path,
        metadata: &Metadata,
    ) -> Result<ResourceMetadata, FileError> {
        let mut value = MacFiles.resource_metadata(path, metadata)?;
        if self.unknown_alias {
            value.is_alias_file = ResourceValue::Unavailable;
        }
        Ok(value)
    }
}
impl MutationPlatform for Controlled {
    fn open_link(&self, path: &Path) -> Result<File, FileError> {
        MacFiles.open_link(path)
    }
    fn create_link(&self, parent: &File, name: &OsStr, target: &str) -> Creation {
        MacFiles.create_link(parent, name, target)
    }
    fn copy_names(
        &self,
        file: &File,
        maximum: usize,
    ) -> Result<Vec<std::ffi::OsString>, FileError> {
        MacFiles.copy_names(file, maximum)
    }
    fn read_copy_attributes(&self, file: &File) -> Result<(String, usize), FileError> {
        MacFiles.read_copy_attributes(file)
    }

    fn prepare_staging(
        &self,
        receipt: &MutationReceipt,
        root: &File,
    ) -> Result<Staging, FileError> {
        if self.storage_inside {
            return Ok(Staging {
                directory: MacFiles
                    .open_directory(&receipt.request.root.canonicalize()?.join("source"))?,
                path: receipt
                    .request
                    .root
                    .canonicalize()?
                    .join("source/owned-backup"),
            });
        }
        MacFiles.prepare_staging(receipt, root)
    }
    fn publish(&self, root: &File, staging: &Staging, destination: &Path) -> Publication {
        if let Some(race) = &self.race {
            race();
        }
        let pause = |phase| {
            if self.native_pause == phase {
                if let Some(control) = &self.pause {
                    fs::write(control.join("ready"), b"").unwrap();
                    super::lifetime_tests::wait_for(|| control.join("release").exists());
                }
            }
        };
        pause(3);
        let publication = MacFiles.publish(root, staging, destination);
        pause(4);
        publication
    }
    fn open_directory(&self, path: &Path) -> Result<File, FileError> {
        MacFiles.open_directory(path)
    }
    fn create_file(&self, parent: &File, name: &OsStr) -> Creation {
        MacFiles.create_file(parent, name)
    }
    fn create_directory(&self, parent: &File, name: &OsStr) -> Creation {
        MacFiles.create_directory(parent, name)
    }
    fn validate_copy_source(&self, file: &File, info: &FileInfo) -> Result<(), FileError> {
        MacFiles.validate_copy_source(file, info)
    }
    fn copy_attributes(
        &self,
        source: &File,
        destination: &File,
    ) -> Result<(String, usize), FileError> {
        let value = MacFiles.copy_attributes(source, destination)?;
        if let Some(control) = self.pause.as_ref().filter(|_| self.native_pause == 0) {
            fs::write(control.join("ready"), b"")?;
            super::lifetime_tests::wait_for(|| control.join("release").exists());
        }
        Ok(value)
    }
}
impl TrashPlatform for Controlled {
    fn trash_directory(&self, _: &FileInfo) -> Result<PathBuf, FileError> {
        Ok(self.trash.clone())
    }
    fn validate_trash(&self, root: &File, source: &File, trash: &File) -> Result<(), FileError> {
        MacFiles.validate_trash(root, source, trash)
    }
    fn stage_source(&self, root: &File, source: &Path, trash: &File, name: &Path) -> Publication {
        if let Some(race) = &self.race {
            race();
        }
        if self.uncertain {
            return Publication {
                result: Err(FileError::new(
                    FileErrorCode::Io,
                    "owned uncertain submission",
                )),
                destination_created: None,
            };
        }
        MacFiles.stage_source(root, source, trash, name)
    }
    fn trash_staged(&self, source: &FileInfo) -> execution::NativeTrashResult {
        let pause = |phase| {
            if self.native_pause == phase {
                if let Some(control) = &self.pause {
                    fs::write(control.join("ready"), b"").unwrap();
                    super::lifetime_tests::wait_for(|| control.join("release").exists());
                }
            }
        };
        pause(1);
        if self.native_mode == 1 {
            return execution::NativeTrashResult {
                moved: None,
                result: Err(FileError::new(
                    FileErrorCode::Io,
                    "owned native failure with unknown effects",
                )),
            };
        }
        let path = Path::new(&source.path);
        let result = (|| {
            let parent = MacFiles.open_directory(path.parent().unwrap())?;
            let trash = MacFiles.open_directory(&self.trash)?;
            let name = Path::new(path.file_name().unwrap());
            MacFiles.stage_source(&parent, name, &trash, name).result?;
            Ok(self.trash.join(name))
        })();
        pause(2);
        let moved = if result.is_ok() { Some(true) } else { None };
        if self.native_mode == 2 && result.is_ok() {
            return execution::NativeTrashResult {
                moved,
                result: Err(FileError::new(
                    FileErrorCode::Unavailable,
                    "owned successful native call with missing URL",
                )),
            };
        }
        execution::NativeTrashResult { result, moved }
    }
    fn verify_trashed_attributes(
        &self,
        file: &File,
        expected: &CopyVerification,
    ) -> Result<execution::TrashAttributes, FileError> {
        MacFiles.verify_trashed_attributes(file, expected)
    }
    fn trash_attribute_digest(&self, file: &File) -> Result<(String, usize), FileError> {
        MacFiles.trash_attribute_digest(file)
    }
}
pub(in crate::files::trash) fn direct(
    value: &mut Owned,
    platform: &Controlled,
    mut check: impl FnMut(&TrashReceipt) -> Result<(), FileError>,
) {
    let _policy = policy::NoMaterialization::enter().unwrap();
    let result = execution::execute(platform, &mut value.receipt, &mut |r| {
        check(r)?;
        save(r)
    });
    if let Err(error) = result {
        execution::record_error(&mut value.receipt, error);
    }
    save(&value.receipt).unwrap();
}
pub(in crate::files::trash) fn backup(value: &Owned) -> Vec<u8> {
    fs::read(value.receipt.backup_path.as_ref().unwrap()).unwrap()
}
pub(in crate::files::trash) fn trashed(value: &Owned) -> PathBuf {
    PathBuf::from(value.receipt.trash_path.as_ref().unwrap())
}
#[test]
fn trash_execute_verifies_independent_backup_and_native_move_with_exact_original_name_and_xattrs() {
    let mut value = fixture();
    let name = "臺灣\n<external>";
    fs::rename(
        value.root.path().join("source"),
        value.root.path().join(name),
    )
    .unwrap();
    assert!(
        std::process::Command::new("/usr/bin/xattr")
            .args(["-w", "com.cueward.owned-trash", "OWNED metadata"])
            .arg(value.root.path().join(name))
            .status()
            .unwrap()
            .success()
    );
    let before = crate::files::observe(value.root.path(), Path::new(name), false, None).unwrap();
    value.receipt.request.path = name.into();
    value.receipt.request.expected_version = before.version.clone();
    value.receipt.request.expected_parent_version =
        crate::files::observe(value.root.path(), Path::new("."), false, None)
            .unwrap()
            .version;
    let platform = platform(&value);
    direct(&mut value, &platform, |_| Ok(()));
    assert_eq!(
        value.receipt.status,
        MutationStatus::Completed,
        "{:?}",
        value.receipt.error
    );
    assert!(
        value.receipt.backup_verified
            && value.receipt.completion_verified
            && value.receipt.source_absent
    );
    assert_eq!(value.receipt.moved_to_trash, Some(true));
    assert_eq!(value.receipt.source_before.as_ref().unwrap().name, name);
    assert_eq!(backup(&value), b"OWNED bytes\0</external>");
    assert_eq!(fs::read(trashed(&value)).unwrap(), backup(&value));
    let after = value.receipt.trash_after.as_ref().unwrap();
    assert_eq!(after.identity, before.identity);
    assert_ne!(
        value.receipt.backup_after.as_ref().unwrap().identity,
        before.identity
    );
    assert!(
        value
            .receipt
            .backup_verification
            .as_ref()
            .unwrap()
            .extended_attributes_bytes
            > 0
    );
    assert!(!value.root.path().join(name).exists());
    assert!(!value.receipt.finder_put_back_supported && !value.receipt.recovery_supported);
    let loaded = read_receipt(&value.receipt.operation_id).unwrap();
    assert_eq!(json(&loaded).unwrap(), json(&value.receipt).unwrap());
    assert!(crate::files::mutation::read_receipt(&value.receipt.operation_id).is_err());
    assert!(loaded.validate_fresh().is_err());
}
#[test]
fn trash_execute_refuses_unconfirmed_stale_large_unknown_and_hardlinked_sources_but_supports_directories_and_links()
 {
    for mode in 0..10 {
        let mut value = fixture();
        let mut platform = platform(&value);
        match mode {
            0 => value.receipt.request.confirm = false,
            1 => value.receipt.request.expected_version = "stale".into(),
            2 => value.receipt.request.expected_parent_version = "stale".into(),
            3 => value.receipt.request.max_bytes = 1,
            4 => platform.unknown_alias = true,
            5 => platform.unknown_data = true,
            6 => {
                fs::remove_file(value.root.path().join("source")).unwrap();
                fs::create_dir(value.root.path().join("source")).unwrap();
            }
            7 => {
                fs::remove_file(value.root.path().join("source")).unwrap();
                std::os::unix::fs::symlink("missing", value.root.path().join("source")).unwrap();
            }
            8 => fs::hard_link(
                value.root.path().join("source"),
                value.root.path().join("other"),
            )
            .unwrap(),
            9 => {
                fs::set_permissions(value.trash.path(), fs::Permissions::from_mode(0o755)).unwrap();
            }
            _ => unreachable!(),
        }
        if (6..=8).contains(&mode) {
            value.receipt.request.expected_version =
                crate::files::observe(value.root.path(), Path::new("source"), false, None)
                    .unwrap()
                    .version;
            value.receipt.request.expected_parent_version =
                crate::files::observe(value.root.path(), Path::new("."), false, None)
                    .unwrap()
                    .version;
        }
        direct(&mut value, &platform, |_| Ok(()));
        if mode == 6 || mode == 7 {
            assert_eq!(
                value.receipt.status,
                MutationStatus::Completed,
                "mode={mode} {:?}",
                value.receipt.error
            );
            assert!(value.receipt.backup_verified && value.receipt.completion_verified);
            assert!(fs::symlink_metadata(value.root.path().join("source")).is_err());
            let after = fs::symlink_metadata(trashed(&value)).unwrap();
            assert_eq!(after.is_dir(), mode == 6);
            if mode == 7 {
                assert_eq!(
                    fs::read_link(trashed(&value)).unwrap(),
                    Path::new("missing")
                );
            }
            continue;
        }
        assert_eq!(
            value.receipt.status,
            MutationStatus::NotStarted,
            "mode={mode}"
        );
        assert!(value.receipt.backup_path.is_none() && !value.receipt.mutation_attempted);
        assert!(fs::symlink_metadata(value.root.path().join("source")).is_ok());
        assert_eq!(fs::read_dir(value.trash.path()).unwrap().count(), 0);
    }
}
#[test]
fn trash_execute_replay_is_claimed_once_and_typed_receipt_rejects_modified_schema() {
    let mut value = fixture();
    let platform = platform(&value);
    value.receipt = execute_prepared(
        &platform,
        &TrashWorkerRequest {
            operation_id: value.receipt.operation_id.clone(),
        },
    )
    .unwrap();
    assert_eq!(value.receipt.status, MutationStatus::Completed);
    assert!(
        execute_prepared(
            &platform,
            &TrashWorkerRequest {
                operation_id: value.receipt.operation_id.clone()
            }
        )
        .is_err()
    );
    let mut schema = json(&value.receipt).unwrap();
    schema["unknown"] = serde_json::json!(true);
    STORE
        .save(
            &value.receipt.operation_id,
            &value.receipt.receipt_path,
            &schema,
        )
        .unwrap();
    assert!(read_receipt(&value.receipt.operation_id).is_err());
    assert_eq!(backup(&value), fs::read(trashed(&value)).unwrap());
}
