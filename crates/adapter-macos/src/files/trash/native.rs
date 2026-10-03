//! Guarded private quarantine followed by Foundation native trash with its actual resulting URL.
use crate::files::{MacFiles, guarded_rename, mutation::attributes};
use cueward_core::files::mutation::{CopyVerification, Publication};
use cueward_core::files::trash::execution::{NativeTrashResult, TrashAttributes, TrashPlatform};
use cueward_core::files::*;
use objc2::rc::autoreleasepool;
use objc2_foundation::{
    NSFileManager, NSSearchPathDirectory, NSSearchPathDomainMask, NSString, NSURL,
};
use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
unsafe extern "C" {
    fn geteuid() -> u32;
}
impl TrashPlatform for MacFiles {
    fn trash_directory(&self, source: &FileInfo) -> Result<PathBuf, FileError> {
        autoreleasepool(|_| {
            let url = NSURL::fileURLWithPath_isDirectory(
                &NSString::from_str(&source.path),
                source.kind == FileKind::Directory,
            );
            let directory = NSFileManager::defaultManager()
                .URLForDirectory_inDomain_appropriateForURL_create_error(
                    NSSearchPathDirectory::TrashDirectory,
                    NSSearchPathDomainMask::UserDomainMask,
                    Some(&url),
                    false,
                )
                .map_err(|error| {
                    unavailable(&format!(
                        "cannot locate existing user Trash: {}",
                        error.localizedDescription()
                    ))
                })?;
            let path = directory
                .path()
                .ok_or_else(|| unavailable("Trash URL has no filesystem path"))?;
            let path = PathBuf::from(path.to_string());
            if !path.is_absolute() || path.canonicalize()? != path {
                return Err(unavailable(
                    "Trash must be an existing canonical non-symlink directory",
                ));
            }
            Ok(path)
        })
    }
    fn validate_trash(&self, root: &File, source: &File, trash: &File) -> Result<(), FileError> {
        let (source, trash) = (source.metadata()?, trash.metadata()?);
        let _ = root;
        // SAFETY: geteuid has no arguments or mutable memory requirements.
        let owner = unsafe { geteuid() };
        if !trash.is_dir()
            || trash.uid() != owner
            || trash.mode() & 0o077 != 0
            || self.stamp(&trash).data_state != DataState::NotDataless
            || source.dev() != trash.dev()
        {
            return Err(unavailable(
                "selected root, source and private user Trash must share an available filesystem",
            ));
        }
        if !(source.is_file() || source.is_dir() || source.file_type().is_symlink())
            || (source.is_file() && source.nlink() != 1)
        {
            return Err(FileError::new(
                FileErrorCode::UnsupportedType,
                "trash execution excludes hard links and non-regular files",
            ));
        }
        Ok(())
    }
    fn stage_source(&self, root: &File, source: &Path, trash: &File, name: &Path) -> Publication {
        let result = guarded_rename::rename(root, source, trash, name);
        let destination_created = guarded_rename::known_outcome(&result);
        Publication {
            result,
            destination_created,
        }
    }
    fn trash_staged(&self, source: &FileInfo) -> NativeTrashResult {
        autoreleasepool(|_| {
            let url = NSURL::fileURLWithPath_isDirectory(
                &NSString::from_str(&source.path),
                source.kind == FileKind::Directory,
            );
            let mut resulting = None;
            match NSFileManager::defaultManager()
                .trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting))
            {
                Err(error) => NativeTrashResult {
                    moved: None,
                    result: Err(unavailable(&format!(
                        "native trash failed ({}:{}): {}",
                        error.domain(),
                        error.code(),
                        error.localizedDescription()
                    ))),
                },
                Ok(()) => NativeTrashResult {
                    moved: Some(true),
                    result: resulting
                        .and_then(|url| url.path())
                        .map(|path| PathBuf::from(path.to_string()))
                        .ok_or_else(|| {
                            unavailable("native trash succeeded but resulting URL is missing")
                        }),
                },
            }
        })
    }
    fn verify_trashed_attributes(
        &self,
        file: &File,
        expected: &CopyVerification,
    ) -> Result<TrashAttributes, FileError> {
        let full = attributes::digest(file)?;
        let exact = full
            == (
                expected.extended_attributes_sha256.clone(),
                expected.extended_attributes_bytes,
            );
        let filtered = attributes::trash_digest(file)?;
        if !exact
            && (!filtered.2
                || filtered.0 != expected.extended_attributes_sha256
                || filtered.1 != expected.extended_attributes_bytes)
        {
            return Err(FileError::new(
                FileErrorCode::VerificationFailed,
                "native Trash changed original xattrs or added an unsupported attribute",
            ));
        }
        if attributes::digest(file)? != full {
            return Err(FileError::new(
                FileErrorCode::Changed,
                "Trash attributes changed during verification",
            ));
        }
        Ok(TrashAttributes {
            sha256: full.0,
            bytes: full.1,
            exact_match: exact,
            accepted_platform_additions: if exact {
                Vec::new()
            } else {
                vec![if full.1 == filtered.1 {
                    "com.apple.macl (added, zero bytes)".into()
                } else {
                    format!("com.apple.macl (added, {} bytes)", full.1 - filtered.1)
                }]
            },
        })
    }
    fn trash_attribute_digest(&self, file: &File) -> Result<(String, usize), FileError> {
        attributes::digest(file)
    }
}
fn unavailable(message: &str) -> FileError {
    FileError::new(FileErrorCode::Unavailable, message)
}
