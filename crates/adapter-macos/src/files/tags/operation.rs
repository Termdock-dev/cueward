use super::super::metadata;
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};

/// Preserve old evidence before any native write and verify observed postconditions.
pub(super) fn execute(
    receipt: &mut TagsReceipt,
    checkpoint: &mut impl FnMut(&TagsReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    let Plan {
        context,
        _lock,
        tags: planned,
        bytes,
    } = prepare(receipt)?;
    receipt.original_attribute_base64 = context
        .stored
        .raw
        .as_ref()
        .map(|bytes| STANDARD.encode(bytes));
    receipt.before = Some(context.before.clone());
    receipt.stage = TagsStage::Prepared;
    checkpoint(receipt)?;
    if planned != context.stored.tags {
        write(&context, &bytes, receipt, checkpoint)?;
    } else {
        receipt.changed_by_operation = Some(false);
    }
    receipt.stage = TagsStage::Verifying;
    checkpoint(receipt)?;
    finish(&context, &planned, receipt)?;
    receipt.stage = TagsStage::Finished;
    receipt.status = TagsStatus::Completed;
    receipt.completion_verified = true;
    checkpoint(receipt)
}
fn write(
    context: &context::Context,
    bytes: &[u8],
    receipt: &mut TagsReceipt,
    checkpoint: &mut impl FnMut(&TagsReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    receipt.stage = TagsStage::Writing;
    receipt.mutation_attempted = true;
    checkpoint(receipt)?;
    let preflight = context
        .revalidate(&receipt.request.root, &receipt.request.path, true)
        .and_then(|_| native::read(&context.file))
        .and_then(|current| {
            if current.version == context.stored.version {
                Ok(())
            } else {
                Err(changed("tags changed before submission"))
            }
        });
    if let Err(error) = preflight {
        receipt.changed_by_operation = Some(false);
        return Err(error);
    }
    match native::write(&context.file, bytes, context.stored.raw.is_some()) {
        Ok(()) => receipt.changed_by_operation = Some(true),
        Err(error) => {
            if matches!(
                error.code,
                FileErrorCode::PermissionDenied | FileErrorCode::Conflict | FileErrorCode::NotFound
            ) {
                receipt.changed_by_operation = Some(false);
            }
            return Err(error);
        }
    }
    checkpoint(receipt)?;
    Ok(())
}
fn finish(
    context: &context::Context,
    planned: &[FileTag],
    receipt: &mut TagsReceipt,
) -> Result<(), FileError> {
    let file = context.revalidate(
        &receipt.request.root,
        &receipt.request.path,
        !receipt.mutation_attempted,
    )?;
    let stored = native::read(&context.file)?;
    if stored.tags != planned {
        return Err(FileError::new(
            FileErrorCode::VerificationFailed,
            "stored tags differ from the requested edit",
        ));
    }
    let resources = metadata::inspect(Path::new(&file.path), &context.file.metadata()?)?;
    context::verify_native_names(&stored, &resources)?;
    let after = context.revalidate(
        &receipt.request.root,
        &receipt.request.path,
        !receipt.mutation_attempted,
    )?;
    if after.version != file.version || native::read(&context.file)?.version != stored.version {
        return Err(changed("target/tags changed during readback"));
    }
    receipt.after = Some(TagSnapshot {
        file: after,
        tags: stored.tags,
        tags_version: stored.version,
    });
    Ok(())
}

fn lock(receipt: &TagsReceipt, context: &context::Context) -> Result<File, FileError> {
    // The lock serializes Cueward editors of this inode, including hardlinked paths.
    // Finder/providers do not participate in this lock or an atomic compare-and-swap.
    let directory = STORE
        .directory(&receipt.operation_id)?
        .parent()
        .ok_or_else(|| changed("missing tag operation storage"))?
        .to_owned();
    let lock_name = {
        use sha2::{Digest, Sha256};
        format!(
            "edit-{:x}.lock",
            Sha256::digest(context.before.file.identity.as_bytes())
        )
    };
    crate::window::lock_path(&directory.join(lock_name))
        .map_err(|e| FileError::new(FileErrorCode::Unavailable, e.to_string()))
}

struct Plan {
    context: context::Context,
    _lock: File,
    tags: Vec<FileTag>,
    bytes: Vec<u8>,
}
fn prepare(receipt: &TagsReceipt) -> Result<Plan, FileError> {
    validate(&receipt.request)?;
    let request = &receipt.request;
    let context = context::Context::prepare(
        &request.root,
        &request.path,
        Some(request.expected_version.clone()),
    )?;
    if context.before.file.kind == FileKind::File {
        use std::os::unix::fs::MetadataExt;
        if context.file.metadata()?.nlink() > 1 {
            return Err(FileError::new(
                FileErrorCode::UnsupportedType,
                "tag edits reject hardlinked files because unselected names would share the change",
            ));
        }
    }
    let lock = lock(receipt, &context)?;
    if context.stored.version != request.expected_tags_version {
        return Err(changed("expected tag revision no longer matches"));
    }
    let planned = edit_tags(&context.stored.tags, &request.edit)?;
    if planned.is_empty() && native::has_legacy_label(&context.file)? {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "removing all stored tags would expose a legacy Finder label; label editing is unsupported",
        ));
    }
    let bytes = codec::encode(&planned)?;
    Ok(Plan {
        context,
        _lock: lock,
        tags: planned,
        bytes,
    })
}
