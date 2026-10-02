//! Prepare and verify private objects; no destination payload writes after publication.
use super::*;

/// Checkpoint the private staging path before creating its candidate object.
pub(super) fn prepare<P: MutationPlatform>(
    platform: &P,
    context: &context::Context<'_, P>,
    receipt: &mut MutationReceipt,
    checkpoint: &mut impl FnMut(&MutationReceipt) -> Result<(), FileError>,
) -> Result<(Staging, File), FileError> {
    let staging = platform.prepare_staging(receipt, &context.root)?;
    receipt.staging_path = Some(crate::files::scope::text(&staging.path)?);
    receipt.stage = MutationStage::Staging;
    checkpoint(receipt)?;
    let name = staging
        .path
        .file_name()
        .ok_or_else(|| context::changed("missing staging leaf"))?;
    let created = match receipt.request.action {
        MutationAction::Mkdir => platform.create_directory(&staging.directory, name),
        MutationAction::Copy { .. } => platform.create_file(&staging.directory, name),
    };
    Ok((staging, created.file?))
}

/// Finish all payload and metadata writes before recording a publishable object.
pub(super) fn populate<P: MutationPlatform>(
    platform: &P,
    context: &context::Context<'_, P>,
    file: &mut File,
    receipt: &mut MutationReceipt,
    checkpoint: &mut impl FnMut(&MutationReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    if let MutationAction::Copy { max_bytes, .. } = receipt.request.action {
        receipt.stage = MutationStage::Copying;
        checkpoint(receipt)?;
        receipt.verification = Some(copying::copy(platform, context, file, max_bytes)?);
    }
    receipt.staging_verified = true;
    receipt.stage = MutationStage::Staged;
    checkpoint(receipt)
}
