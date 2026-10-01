use super::*;
use crate::files::MacFiles;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

/// Copy only guarded local bytes, binding the descriptor to the observed revision.
pub(super) fn create(
    file: &FileInfo,
    directory: &Path,
    maximum: u64,
) -> Result<(PathBuf, String), FileError> {
    if file.kind != FileKind::File {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "preview requires a regular file; packages and unfollowed links are unsupported",
        ));
    }
    if file.data_state != DataState::NotDataless {
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            "preview does not download placeholder data",
        ));
    }
    if file.size > maximum {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "source exceeds max-input-bytes",
        ));
    }
    let mut source = MacFiles.open_regular(Path::new(&file.path))?;
    check(&source, file)?;
    let extension = Path::new(&file.name)
        .extension()
        .and_then(|s| s.to_str())
        .filter(|s| s.len() <= 32 && s.bytes().all(|b| b.is_ascii_alphanumeric()));
    let input = directory.join(extension.map_or("input".into(), |s| format!("input.{s}")));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&input)?;
    let (bytes, hash) = copy(&mut source, &mut output, maximum)?;
    if bytes != file.size {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "source size changed during preview snapshot",
        ));
    }
    check(&source, file)?;
    Ok((input, hash))
}

fn check(source: &File, before: &FileInfo) -> Result<(), FileError> {
    let stamp = MacFiles.stamp(&source.metadata()?);
    if stamp.version != before.version || stamp.data_state != DataState::NotDataless {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "source descriptor changed during preview snapshot",
        ));
    }
    Ok(())
}

fn copy(input: &mut File, output: &mut File, maximum: u64) -> Result<(u64, String), FileError> {
    let mut hasher = Sha256::new();
    let mut total = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let size = input.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        total += size as u64;
        if total > maximum {
            return Err(FileError::new(
                FileErrorCode::ScanLimit,
                "source grew beyond max-input-bytes",
            ));
        }
        output.write_all(&buffer[..size])?;
        hasher.update(&buffer[..size]);
    }
    output.flush()?;
    Ok((total, format!("{:x}", hasher.finalize())))
}
