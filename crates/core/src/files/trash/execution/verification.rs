use super::*;
use std::io::{Seek, SeekFrom};

pub(super) struct Backup {
    pub storage: Staging,
    pub file: File,
}
/// Build a private independent copy. It is never deleted, even after success.
pub(super) fn backup<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut TrashReceipt,
    checkpoint: &mut impl FnMut(&TrashReceipt) -> Result<(), FileError>,
) -> Result<Backup, FileError> {
    let storage = prepare_backup(context, receipt, checkpoint)?;
    let leaf = storage
        .path
        .file_name()
        .ok_or_else(|| changed("backup leaf missing"))?;
    let mut file = context
        .platform
        .create_file(&storage.directory, leaf)
        .file?;
    let verification = copy_file_verified(
        context.platform,
        &context.source,
        &context.plan.source,
        &mut file,
        receipt.request.max_bytes,
    )?;
    receipt.backup_after = Some(observe(context.platform, &storage.path)?);
    receipt.backup_verification = Some(verification);
    let backup = Backup { storage, file };
    check_backup(context, receipt, &backup)?;
    Ok(backup)
}
pub(super) fn check_backup<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &TrashReceipt,
    backup: &Backup,
) -> Result<(), FileError> {
    let info = observe(context.platform, &backup.storage.path)?;
    let before = receipt
        .backup_after
        .as_ref()
        .ok_or_else(|| changed("backup observation missing"))?;
    let verification = receipt
        .backup_verification
        .as_ref()
        .ok_or_else(|| changed("backup verification missing"))?;
    if info.path != before.path
        || info.version != before.version
        || context.platform.stamp(&backup.file.metadata()?).version != info.version
    {
        return Err(changed("verified backup was replaced or changed"));
    }
    verify_file(context.platform, &info, verification)?;
    Ok(())
}
pub(super) fn verify_file(
    platform: &impl TrashPlatform,
    info: &FileInfo,
    verification: &CopyVerification,
) -> Result<(), FileError> {
    let file = platform.open_regular(Path::new(&info.path))?;
    verify_readable(platform, &file, info, verification)?;
    if platform.trash_attribute_digest(&file)?
        != (
            verification.extended_attributes_sha256.clone(),
            verification.extended_attributes_bytes,
        )
    {
        return Err(FileError::new(
            FileErrorCode::VerificationFailed,
            "file extended attributes differ from verified backup",
        ));
    }
    Ok(())
}
pub(super) fn finish<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut TrashReceipt,
    backup: &Backup,
) -> Result<(), FileError> {
    context.check_trash(false)?;
    check_backup(context, receipt, backup)?;
    let path = PathBuf::from(
        receipt
            .trash_path
            .as_ref()
            .ok_or_else(|| changed("native Trash URL unavailable"))?,
    );
    if path.parent() != Some(Path::new(&context.trash_info.path)) {
        return Err(changed(
            "native resulting item is not in the verified Trash directory",
        ));
    }
    let after = observe(context.platform, &path)?;
    receipt.trash_after = Some(after.clone());
    verify_moved(context, receipt, &path, &after)?;
    observe_source_absence(context, receipt)?;
    check_staging_absent(receipt)?;
    let stable = observe(context.platform, &path)?;
    if stable.version != after.version {
        return Err(changed("Trash entry changed during verification"));
    }
    context.check_trash(false)?;
    bound(receipt)
}
pub(super) fn observe_source_absence<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut TrashReceipt,
) -> Result<(), FileError> {
    receipt.source_absent = false;
    let scope = Scope::new(context.platform, &receipt.request.root)?;
    let root = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
    if root.path != context.plan.root.path
        || root.identity != context.plan.root.identity
        || root.version != context.platform.stamp(&context.root.metadata()?).version
    {
        return Err(changed("source root changed after move"));
    }
    let parent_path = receipt
        .request
        .path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = scope.info(&scope.resolve(parent_path, false, false)?)?;
    if parent.path != context.plan.source_parent.path
        || parent.identity != context.plan.source_parent.identity
    {
        return Err(changed("source parent changed after move"));
    }
    match scope.resolve(&receipt.request.path, false, true) {
        Err(error) if error.code == FileErrorCode::NotFound => receipt.source_absent = true,
        Err(error) => return Err(error),
        Ok(_) => {
            return Err(changed(
                "source path occupied after move; never overwrite or repair",
            ));
        }
    }
    receipt.root_after = Some(root);
    receipt.parent_after = Some(parent);
    scope.revalidate_root()
}

fn prepare_backup<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut TrashReceipt,
    checkpoint: &mut impl FnMut(&TrashReceipt) -> Result<(), FileError>,
) -> Result<Staging, FileError> {
    let proxy = MutationReceipt::new(
        receipt.operation_id.clone(),
        MutationRequest {
            root: receipt.request.root.clone(),
            destination: receipt.request.path.clone(),
            expected_parent_version: receipt.request.expected_parent_version.clone(),
            action: MutationAction::Mkdir,
        },
        receipt.receipt_path.clone(),
    );
    let storage = context.platform.prepare_staging(&proxy, &context.root)?;
    let directory = storage
        .path
        .parent()
        .ok_or_else(|| changed("backup parent missing"))?
        .canonicalize()?;
    if directory.starts_with(&context.plan.root.path)
        || directory.starts_with(&context.trash_info.path)
    {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "backup store must be outside selected root and Trash",
        ));
    }
    receipt.backup_path = Some(text(&storage.path)?);
    receipt.stage = TrashStage::BackingUp;
    bound(receipt)?;
    checkpoint(receipt)?;
    context.revalidate(receipt)?;
    Ok(storage)
}

fn verify_moved<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut TrashReceipt,
    path: &Path,
    after: &FileInfo,
) -> Result<(), FileError> {
    if after.identity != context.plan.source.identity
        || after.kind != FileKind::File
        || after.data_state != DataState::NotDataless
        || after.size != context.plan.source.size
        || after.mode != context.plan.source.mode
        || after.modified != context.plan.source.modified
        || context.platform.stamp(&context.source.metadata()?).version != after.version
    {
        return Err(changed(
            "moved object differs from selected file; keep Trash entry and backup",
        ));
    }
    let verification = receipt
        .backup_verification
        .as_ref()
        .ok_or_else(|| changed("missing backup digest"))?;
    let file = context.platform.open_regular(path)?;
    verify_readable(context.platform, &file, after, verification)?;
    receipt.trash_attributes = Some(
        context
            .platform
            .verify_trashed_attributes(&file, verification)?,
    );
    // Rename changes ctime; identity/data/mtime/mode, not the old full version, must match.
    let mut held = &context.source;
    held.seek(SeekFrom::Start(0))?;
    verify_readable(
        context.platform,
        &context.source,
        after,
        receipt
            .backup_verification
            .as_ref()
            .ok_or_else(|| changed("missing backup digest"))?,
    )?;
    Ok(())
}
fn check_staging_absent(receipt: &mut TrashReceipt) -> Result<(), FileError> {
    match std::fs::symlink_metadata(
        receipt
            .staging_path
            .as_ref()
            .ok_or_else(|| changed("staging path missing"))?,
    ) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            receipt.staged_source_absent = true
        }
        Err(error) => return Err(error.into()),
        Ok(_) => {
            return Err(changed(
                "private source path remains occupied after native trash",
            ));
        }
    }
    Ok(())
}
