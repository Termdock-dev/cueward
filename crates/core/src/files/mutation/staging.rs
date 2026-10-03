//! Prepare and verify private objects; no destination payload writes after publication.
use super::*;

/// Checkpoint the private staging path before creating its candidate object.
pub(super) fn prepare<P: MutationPlatform>(
    platform: &P,
    context: &mut context::Context<'_, P>,
    receipt: &mut MutationReceipt,
    checkpoint: &mut impl FnMut(&MutationReceipt) -> Result<(), FileError>,
) -> Result<(Staging, File), FileError> {
    let staging = platform.prepare_staging(receipt, &context.root)?;
    if context.source.as_ref().is_some_and(|source| {
        source.info.kind == FileKind::Directory && staging.path.starts_with(&source.info.path)
    }) {
        return Err(FileError::new(
            FileErrorCode::Conflict,
            "private staging must not be inside source tree",
        ));
    }
    let mut root_info = receipt
        .root_before
        .clone()
        .ok_or_else(|| context::changed("missing root"))?;
    super::objects::advance_staging(
        platform,
        &staging,
        &receipt.request.root,
        &context.root,
        &context.parent,
        &mut root_info,
        &mut context.parent_info,
    )?;
    receipt.staging_path = Some(crate::files::scope::text(&staging.path)?);
    receipt.stage = MutationStage::Staging;
    checkpoint(receipt)?;
    let name = staging
        .path
        .file_name()
        .ok_or_else(|| context::changed("missing staging leaf"))?;
    let created = match receipt.request.action {
        MutationAction::Mkdir => platform.create_directory(&staging.directory, name),
        MutationAction::Copy { .. } | MutationAction::Duplicate { .. } => platform.create_object(
            &staging.directory,
            name,
            &context
                .source
                .as_ref()
                .ok_or_else(|| context::changed("missing source"))?
                .info,
        ),
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
    if let MutationAction::Copy { max_bytes, .. } | MutationAction::Duplicate { max_bytes, .. } =
        receipt.request.action
    {
        receipt.stage = MutationStage::Copying;
        checkpoint(receipt)?;
        receipt.verification = Some(copying::copy(platform, context, file, max_bytes)?);
        verify_private(platform, context, receipt)?;
    }
    receipt.staging_verified = true;
    receipt.stage = MutationStage::Staged;
    checkpoint(receipt)
}

fn verify_private<P: MutationPlatform>(
    platform: &P,
    context: &context::Context<'_, P>,
    receipt: &MutationReceipt,
) -> Result<(), FileError> {
    let source = context
        .source
        .as_ref()
        .ok_or_else(|| context::changed("missing source"))?;
    if source.info.kind == FileKind::File {
        return Ok(());
    }
    let path = Path::new(
        receipt
            .staging_path
            .as_ref()
            .ok_or_else(|| context::changed("missing staging path"))?,
    );
    let scope = crate::files::scope::Scope::new(
        platform,
        path.parent()
            .ok_or_else(|| context::changed("missing staging parent"))?,
    )?;
    let leaf = path
        .file_name()
        .ok_or_else(|| context::changed("missing staging leaf"))?;
    let info = scope.info(&scope.resolve(Path::new(leaf), false, true)?)?;
    let proof = receipt
        .verification
        .as_ref()
        .ok_or_else(|| context::changed("missing copy proof"))?;
    if info.kind != source.info.kind
        || info.mode != source.info.mode
        || info.modified != source.info.modified
    {
        return Err(context::changed("private copy metadata differs"));
    }
    let held = platform.open_object(&info)?;
    verify_readable(platform, &held, &info, proof)?;
    if platform.read_copy_attributes(&held)?
        != (
            proof.extended_attributes_sha256.clone(),
            proof.extended_attributes_bytes,
        )
    {
        return Err(context::changed("private copy attributes differ"));
    }
    scope.revalidate_root()
}

pub(super) fn verify_before_publish<P: MutationPlatform>(
    platform: &P,
    context: &context::Context<'_, P>,
    receipt: &MutationReceipt,
) -> Result<(), FileError> {
    if let Some(source) = &context.source {
        if source.info.kind != FileKind::File {
            let proof = receipt
                .verification
                .as_ref()
                .ok_or_else(|| context::changed("missing copy proof"))?;
            verify_readable(platform, &source.file, &source.info, proof)?;
            verify_private(platform, context, receipt)?;
        }
    }
    Ok(())
}
