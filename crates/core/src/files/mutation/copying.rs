use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom, Write};

pub(super) fn copy<P: MutationPlatform>(
    platform: &P,
    context: &context::Context<'_, P>,
    destination: &mut File,
    maximum: u64,
) -> Result<CopyVerification, FileError> {
    let source = context
        .source
        .as_ref()
        .ok_or_else(|| context::changed("missing source descriptor"))?;
    let mut reader = &source.file;
    let (bytes, digest) = transfer(&mut reader, destination, maximum)?;
    if bytes != source.info.size {
        return Err(context::changed("source size changed during copy"));
    }
    context::check_source(platform, &source.file, &source.info)?;
    let (attributes, attribute_bytes) = platform.copy_attributes(&source.file, destination)?;
    let metadata = source.file.metadata()?;
    destination.set_permissions(metadata.permissions())?;
    destination.set_times(std::fs::FileTimes::new().set_modified(metadata.modified()?))?;
    destination.sync_all()?;
    context::check_source(platform, &source.file, &source.info)?;
    destination.seek(SeekFrom::Start(0))?;
    let (read_bytes, read_digest) = hash(destination, maximum)?;
    let actual = destination.metadata()?;
    let permissions_equal = actual.permissions() == metadata.permissions();
    let modified_equal = actual.modified()? == metadata.modified()?;
    if read_bytes != bytes || read_digest != digest || !permissions_equal || !modified_equal {
        return Err(FileError::new(
            FileErrorCode::VerificationFailed,
            "destination content/permissions/mtime verification failed",
        ));
    }
    Ok(CopyVerification {
        content_sha256: digest,
        bytes,
        permissions_equal,
        modified_equal,
        extended_attributes_sha256: attributes,
        extended_attributes_bytes: attribute_bytes,
    })
}

fn transfer(
    input: &mut impl Read,
    output: &mut impl Write,
    maximum: u64,
) -> Result<(u64, String), FileError> {
    let mut total = 0;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let bytes = input.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        total += bytes as u64;
        if total > maximum {
            return Err(FileError::new(
                FileErrorCode::ScanLimit,
                "source grew beyond max-bytes",
            ));
        }
        output.write_all(&buffer[..bytes])?;
        hash.update(&buffer[..bytes]);
    }
    Ok((total, format!("{:x}", hash.finalize())))
}

fn hash(input: &mut impl Read, maximum: u64) -> Result<(u64, String), FileError> {
    transfer(input, &mut std::io::sink(), maximum)
}

pub(super) fn verify_readable(
    platform: &impl MutationPlatform,
    file: &File,
    info: &FileInfo,
    verification: &CopyVerification,
) -> Result<(), FileError> {
    context::check_source(platform, file, info)?;
    let (bytes, digest) = hash(&mut &*file, verification.bytes)?;
    context::check_source(platform, file, info)?;
    if bytes != verification.bytes || digest != verification.content_sha256 {
        return Err(FileError::new(
            FileErrorCode::VerificationFailed,
            "reopened destination content differs",
        ));
    }
    Ok(())
}
