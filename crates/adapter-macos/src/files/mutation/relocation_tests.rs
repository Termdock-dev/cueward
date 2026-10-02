//! Real nested-directory relocation while the final read-back descriptor stays valid.
use super::*;
use std::cell::Cell;
use std::ffi::OsStr;
use std::fs::File;

#[derive(Clone, Copy)]
enum Replacement {
    Missing,
    Directory,
    Symlink,
}
struct RelocatingParent {
    destination: PathBuf,
    moved: PathBuf,
    replacement: Replacement,
    triggered: Cell<bool>,
}
impl FilePlatform for RelocatingParent {
    fn stamp(&self, metadata: &fs::Metadata) -> FileStamp {
        MacFiles.stamp(metadata)
    }
    fn open_regular(&self, path: &Path) -> std::io::Result<File> {
        let file = MacFiles.open_regular(path)?;
        if path == self.destination && !self.triggered.replace(true) {
            let parent = path.parent().unwrap();
            // The already-open file remains readable at its moved path. The root itself
            // is unchanged because relocation happens one directory below the root.
            fs::rename(parent, &self.moved)?;
            match self.replacement {
                Replacement::Missing => {}
                Replacement::Directory => {
                    fs::create_dir(parent)?;
                    fs::write(path, b"owned replacement bytes")?;
                }
                Replacement::Symlink => symlink(&self.moved, parent)?,
            }
        }
        Ok(file)
    }
}
impl MutationPlatform for RelocatingParent {
    fn prepare_staging(
        &self,
        receipt: &MutationReceipt,
        root: &File,
    ) -> Result<Staging, FileError> {
        MacFiles.prepare_staging(receipt, root)
    }
    fn publish(&self, root: &File, staging: &Staging, destination: &Path) -> Publication {
        MacFiles.publish(root, staging, destination)
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
        MacFiles.copy_attributes(source, destination)
    }
}
fn prepare_nested(
    replacement: Replacement,
) -> (tempfile::TempDir, MutationRequest, RelocatingParent) {
    let (root, mut request) = fixture();
    fs::create_dir_all(root.path().join("nested/parent")).unwrap();
    request.destination = "nested/parent/copied.txt".into();
    fresh(&mut request);
    let platform = RelocatingParent {
        destination: root
            .path()
            .canonicalize()
            .unwrap()
            .join(&request.destination),
        moved: root.path().canonicalize().unwrap().join("nested/moved"),
        replacement,
        triggered: Cell::new(false),
    };
    (root, request, platform)
}
fn nested_copy(replacement: Replacement) {
    let (root, request, platform) = prepare_nested(replacement);
    let root_stamp = MacFiles.stamp(&fs::metadata(root.path()).unwrap());
    let source = fs::read(root.path().join("source.txt")).unwrap();
    let mut receipt = journal::create(&request).unwrap();
    if let Err(error) =
        cueward_core::files::mutation::execute(&platform, &mut receipt, &mut journal::save)
    {
        record_error(&mut receipt, error);
    }
    journal::save(&receipt).unwrap();
    let saved = journal::load(&receipt.operation_id).unwrap();
    fs::remove_dir_all(journal::directory(&receipt.operation_id).unwrap()).unwrap();
    assert!(
        platform.triggered.get(),
        "must relocate after reopening the final read-back file: {:?}",
        saved.error
    );
    assert_eq!(
        MacFiles.stamp(&fs::metadata(root.path()).unwrap()),
        root_stamp
    );
    assert_eq!(fs::read(platform.moved.join("copied.txt")).unwrap(), source);
    assert_eq!(fs::read(root.path().join("source.txt")).unwrap(), source);
    check_replacement(&platform);
    assert_eq!(
        saved.status,
        MutationStatus::Incomplete,
        "{:?}",
        saved.error
    );
    assert!(!saved.completion_verified);
    assert_eq!(saved.destination_created, Some(true));
    assert!(saved.error.is_some());
}
fn check_replacement(platform: &RelocatingParent) {
    match platform.replacement {
        Replacement::Missing => assert!(!platform.destination.exists()),
        Replacement::Directory => assert_eq!(
            fs::read(&platform.destination).unwrap(),
            b"owned replacement bytes"
        ),
        Replacement::Symlink => assert_eq!(
            fs::read_link(platform.destination.parent().unwrap()).unwrap(),
            platform.moved
        ),
    }
}
#[test]
fn final_readback_rejects_a_relocated_nested_parent() {
    nested_copy(Replacement::Missing);
}
#[test]
fn final_readback_rejects_an_imposter_parent_and_preserves_its_file() {
    nested_copy(Replacement::Directory);
}
#[test]
fn final_readback_rejects_a_parent_replaced_by_a_symlink() {
    nested_copy(Replacement::Symlink);
}
