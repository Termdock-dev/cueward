use super::*;

/// Verify an independent candidate in separate private same-volume staging.
pub(super) fn prepare<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut RestoreReceipt,
    checkpoint: &mut impl FnMut(&RestoreReceipt) -> Result<(), FileError>,
) -> Result<(Staging, File), FileError> {
    let storage = storage(context, receipt)?;
    receipt.staging_path = Some(text(&storage.path)?);
    receipt.stage = RestoreStage::Copying;
    bound(receipt, 128 * 1024)?;
    checkpoint(receipt)?;
    context.revalidate(receipt, false)?;
    let leaf = storage
        .path
        .file_name()
        .ok_or_else(|| changed("staging leaf missing"))?;
    let mut candidate = context
        .platform
        .create_file(&storage.directory, leaf)
        .file?;
    let proof = copy_file_verified(
        context.platform,
        &context.backup,
        &context.backup_info,
        &mut candidate,
        context.proof()?.bytes,
    )?;
    if value(&proof)? != value(context.proof()?)? {
        return Err(changed(
            "restored copy differs from recorded backup verification",
        ));
    }
    receipt.verification = Some(proof);
    receipt.staging_after = Some(observe(context.platform, &storage.path)?);
    verify_candidate(context, receipt, &candidate, &storage.path)?;
    bound(receipt, 128 * 1024)?;
    Ok((storage, candidate))
}
/// Read back the named candidate and check its held identity and original metadata.
pub(super) fn verify_candidate<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &RestoreReceipt,
    candidate: &File,
    path: &Path,
) -> Result<FileInfo, FileError> {
    let now = observe(context.platform, path)?;
    let before = receipt
        .staging_after
        .as_ref()
        .ok_or_else(|| changed("candidate evidence missing"))?;
    if now.identity != before.identity
        || context.platform.stamp(&candidate.metadata()?).version != now.version
        || now.mode != context.backup_info.mode
        || now.modified != context.backup_info.modified
        || now.size != context.backup_info.size
        || now.kind != FileKind::File
        || now.data_state != DataState::NotDataless
    {
        return Err(changed("restored candidate path or metadata changed"));
    }
    verify_file(context.platform, &now, context.proof()?)?;
    Ok(now)
}
/// Verify the original-path result and retained backup before recording completion.
pub(super) fn finish<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut RestoreReceipt,
    candidate: &File,
) -> Result<(), FileError> {
    let (root, parent) = context.revalidate(receipt, true)?;
    let path = Path::new(&context.root_info.path).join(&context.destination);
    let after = verify_candidate(context, receipt, candidate, &path)?;
    let backup = context.check_backup()?;
    let (final_root, final_parent) = context.revalidate(receipt, true)?;
    let stable = observe(context.platform, &path)?;
    if root.version != final_root.version
        || parent.version != final_parent.version
        || stable.version != after.version
    {
        return Err(changed(
            "restore destination or scope changed during final verification",
        ));
    }
    receipt.root_after = Some(final_root);
    receipt.parent_after = Some(final_parent);
    receipt.destination_after = Some(stable);
    receipt.backup_after = Some(backup);
    bound(receipt, 128 * 1024)
}

fn storage<P: TrashPlatform>(
    context: &context::Context<'_, P>,
    receipt: &RestoreReceipt,
) -> Result<Staging, FileError> {
    let proxy = MutationReceipt::new(
        receipt.operation_id.clone(),
        MutationRequest {
            root: receipt.request.root.clone(),
            destination: context.destination.clone(),
            expected_parent_version: receipt.request.expected_parent_version.clone(),
            action: MutationAction::Mkdir,
        },
        receipt.receipt_path.clone(),
    );
    let storage = context.platform.prepare_staging(&proxy, &context.root)?;
    let parent = storage
        .path
        .parent()
        .ok_or_else(|| changed("restore staging parent missing"))?
        .canonicalize()?;
    if parent.starts_with(&context.root_info.path)
        || parent
            == Path::new(&context.backup_info.path)
                .parent()
                .ok_or_else(|| changed("backup parent missing"))?
    {
        return Err(invalid(
            "restore staging must be separate from selected root and original backup operation",
        ));
    }
    Ok(storage)
}
