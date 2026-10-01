use super::*;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};

pub(super) fn root() -> Result<PathBuf, FileError> {
    let home = std::env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            FileError::new(
                FileErrorCode::Unavailable,
                "HOME is required for operation receipts",
            )
        })?;
    let path = PathBuf::from(home).join(".cueward/operations/files");
    if !path.is_absolute() {
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            "HOME must be absolute",
        ));
    }
    fs::create_dir_all(&path)?;
    Ok(path.canonicalize()?)
}
pub(super) fn directory(id: &str) -> Result<PathBuf, FileError> {
    if uuid::Uuid::parse_str(id).is_err() || id.len() != 36 || id.to_ascii_lowercase() != id {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "operation-id must be a canonical UUID",
        ));
    }
    let path = root()?.join(id);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_dir() || metadata.mode() & 0o777 != 0o700 {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "operation receipt directory must be private and not a symlink",
        ));
    }
    Ok(path)
}
pub(super) fn create(request: &MutationRequest) -> Result<MutationReceipt, FileError> {
    let id = uuid::Uuid::new_v4().to_string();
    let directory = root()?.join(&id);
    fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let path = directory.join("receipt.json");
    let text = path
        .to_str()
        .ok_or_else(|| {
            FileError::new(
                FileErrorCode::UnsupportedPathEncoding,
                "receipt path must be UTF-8",
            )
        })?
        .to_owned();
    let receipt = MutationReceipt::new(id, request.clone(), text);
    save(&receipt)?;
    Ok(receipt)
}
pub(super) fn save(receipt: &MutationReceipt) -> Result<(), FileError> {
    let directory = directory(&receipt.operation_id)?;
    let path = directory.join("receipt.json");
    if path.to_str() != Some(&receipt.receipt_path) {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "receipt path mismatch",
        ));
    }
    let bytes = serde_json::to_vec_pretty(receipt).map_err(internal)?;
    let mut temp = tempfile::NamedTempFile::new_in(&directory)?;
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(&path).map_err(|e| FileError::from(e.error))?;
    Ok(())
}
pub(super) fn load(id: &str) -> Result<MutationReceipt, FileError> {
    let path = directory(id)?.join("receipt.json");
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
    let receipt: MutationReceipt = serde_json::from_slice(&bytes).map_err(internal)?;
    if receipt.operation_id != id || path.to_str() != Some(&receipt.receipt_path) {
        return Err(internal("receipt identity/path mismatch"));
    }
    Ok(receipt)
}
pub(super) fn claim(id: &str) -> Result<(), FileError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory(id)?.join("claimed"))?
        .sync_all()?;
    Ok(())
}
fn internal(message: impl std::fmt::Display) -> FileError {
    FileError::new(FileErrorCode::Internal, message.to_string())
}
