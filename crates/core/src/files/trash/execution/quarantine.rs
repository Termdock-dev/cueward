use super::verification::{Backup, check_backup, observe_source_absence, verify_file};
use super::*;
/// Remove from the selected namespace only after a verified backup exists.
pub(super) fn quarantine<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut TrashReceipt,
    backup: &Backup,
    checkpoint: &mut impl FnMut(&TrashReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    let relative = prepare_quarantine(context, receipt, backup, checkpoint)?;
    context.revalidate(receipt)?;
    check_backup(context, receipt, backup)?;
    receipt.mutation_attempted = true;
    checkpoint(receipt)?;
    context
        .revalidate(receipt)
        .inspect_err(|_| receipt.source_removed = Some(false))?;
    let moved = context.platform.stage_source(
        &context.root,
        &receipt.request.path,
        &backup.storage.directory,
        &relative,
    );
    receipt.source_removed = moved.destination_created;
    moved.result?;
    let after = observe(
        context.platform,
        Path::new(
            receipt
                .staging_path
                .as_ref()
                .ok_or_else(|| changed("missing staging path"))?,
        ),
    )?;
    receipt.staged_after = Some(after);
    checkpoint(receipt)?;
    check_staged(context, receipt)
}
pub(super) fn check_staged<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut TrashReceipt,
) -> Result<(), FileError> {
    let path = Path::new(
        receipt
            .staging_path
            .as_ref()
            .ok_or_else(|| changed("missing staging path"))?,
    );
    let after = observe(context.platform, path)?;
    let before = receipt
        .staged_after
        .as_ref()
        .ok_or_else(|| changed("missing staged observation"))?;
    if after.path != before.path
        || after.version != before.version
        || after.identity != context.plan.source.identity
        || after.kind != FileKind::File
        || after.data_state != DataState::NotDataless
        || after.size != context.plan.source.size
        || after.mode != context.plan.source.mode
        || after.modified != context.plan.source.modified
        || context.platform.stamp(&context.source.metadata()?).version != after.version
    {
        return Err(changed(
            "staged object differs from selected file; keep both staging and backup",
        ));
    }
    verify_file(
        context.platform,
        &after,
        receipt
            .backup_verification
            .as_ref()
            .ok_or_else(|| changed("missing backup digest"))?,
    )?;
    observe_source_absence(context, receipt)
}
pub(super) fn native_trash<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut TrashReceipt,
    backup: &Backup,
    checkpoint: &mut impl FnMut(&TrashReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    receipt.stage = TrashStage::Quarantined;
    checkpoint(receipt)?;
    check_staged(context, receipt)?;
    check_backup(context, receipt, backup)?;
    context.check_trash(true)?;
    receipt.stage = TrashStage::Trashing;
    receipt.trash_attempted = true;
    checkpoint(receipt)?;
    let checks = (|| {
        check_staged(context, receipt)?;
        check_backup(context, receipt, backup)?;
        context.check_trash(true)
    })();
    checks.inspect_err(|_| receipt.moved_to_trash = Some(false))?;
    let source = receipt
        .staged_after
        .as_ref()
        .ok_or_else(|| changed("missing staged source"))?;
    let native = context.platform.trash_staged(source);
    receipt.moved_to_trash = native.moved;
    receipt.trash_path = native.result.as_ref().ok().map(|p| text(p)).transpose()?;
    native.result?;
    Ok(())
}

fn prepare_quarantine<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut TrashReceipt,
    backup: &Backup,
    checkpoint: &mut impl FnMut(&TrashReceipt) -> Result<(), FileError>,
) -> Result<PathBuf, FileError> {
    receipt.stage = TrashStage::Quarantining;
    let relative = PathBuf::from("trash-source").join(&context.plan.source.name);
    receipt.staging_path = Some(text(
        &backup
            .storage
            .path
            .parent()
            .ok_or_else(|| changed("missing private parent"))?
            .join(&relative),
    )?);
    bound(receipt)?;
    checkpoint(receipt)?;
    context
        .platform
        .create_directory(
            &backup.storage.directory,
            std::ffi::OsStr::new("trash-source"),
        )
        .file?;
    Ok(relative)
}
