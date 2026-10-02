//! Private bounded evidence shared by file-creation and tag-edit workers.
use super::{FilePlatform, MacFiles};
use cueward_core::files::*;
use serde::{Serialize, de::DeserializeOwned};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::PathBuf;

pub(super) struct Store(pub &'static str);
impl Store {
    fn root(&self) -> Result<PathBuf, FileError> {
        let home = std::env::var_os("HOME")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                FileError::new(
                    FileErrorCode::Unavailable,
                    "HOME is required for operation receipts",
                )
            })?;
        let path = PathBuf::from(home).join(".cueward/operations").join(self.0);
        if !path.is_absolute() {
            return Err(FileError::new(
                FileErrorCode::Unavailable,
                "HOME must be absolute",
            ));
        }
        fs::create_dir_all(&path)?;
        Ok(path.canonicalize()?)
    }
    /// Validate a private non-symlink operation directory and canonical UUID.
    pub(super) fn directory(&self, id: &str) -> Result<PathBuf, FileError> {
        if uuid::Uuid::parse_str(id).is_err() || id.len() != 36 || id.to_ascii_lowercase() != id {
            return Err(invalid("operation-id must be a canonical UUID"));
        }
        let path = self.root()?.join(id);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_dir() || metadata.mode() & 0o777 != 0o700 {
            return Err(invalid(
                "operation receipt directory must be private and not a symlink",
            ));
        }
        Ok(path)
    }
    /// Allocate a new private operation and immediately persist its initial record.
    pub(super) fn create<T: Serialize>(
        &self,
        build: impl FnOnce(String, String) -> T,
    ) -> Result<T, FileError> {
        let id = uuid::Uuid::new_v4().to_string();
        let directory = self.root()?.join(&id);
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let path = directory
            .join("receipt.json")
            .to_str()
            .ok_or_else(|| {
                FileError::new(
                    FileErrorCode::UnsupportedPathEncoding,
                    "receipt path must be UTF-8",
                )
            })?
            .to_owned();
        let value = build(id.clone(), path.clone());
        self.save(&id, &path, &value)?;
        Ok(value)
    }
    /// Sync and atomically replace private evidence; no payload cleanup occurs.
    pub(super) fn save(
        &self,
        id: &str,
        expected_path: &str,
        value: &impl Serialize,
    ) -> Result<(), FileError> {
        let directory = self.directory(id)?;
        let path = directory.join("receipt.json");
        if path.to_str() != Some(expected_path) {
            return Err(invalid("receipt path mismatch"));
        }
        let bytes = serde_json::to_vec_pretty(value).map_err(internal)?;
        if bytes.len() > 256 * 1024 {
            return Err(internal("receipt exceeds bounds"));
        }
        let mut temp = tempfile::NamedTempFile::new_in(&directory)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(&path).map_err(|e| FileError::from(e.error))?;
        Ok(())
    }
    /// Load bounded evidence through guarded descriptors; callers verify record identity.
    pub(super) fn load<T: DeserializeOwned>(&self, id: &str) -> Result<T, FileError> {
        let path = self.directory(id)?.join("receipt.json");
        let mut file = MacFiles.open_regular(&path)?;
        if !file.metadata()?.is_file() || file.metadata()?.len() > 256 * 1024 {
            return Err(internal("receipt exceeds bounds or is not regular"));
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(256 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 256 * 1024 {
            return Err(internal("receipt grew beyond bounds"));
        }
        serde_json::from_slice(&bytes).map_err(internal)
    }
    /// Claim an operation exactly once, without replacing an existing claim.
    pub(super) fn claim(&self, id: &str) -> Result<(), FileError> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.directory(id)?.join("claimed"))?
            .sync_all()?;
        Ok(())
    }
}
fn invalid(message: &str) -> FileError {
    FileError::new(FileErrorCode::InvalidOptions, message)
}
fn internal(message: impl std::fmt::Display) -> FileError {
    FileError::new(FileErrorCode::Internal, message.to_string())
}
