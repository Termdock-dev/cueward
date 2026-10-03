use super::*;
use crate::files::scope::Scope;
use context::Context;

/// Collect actual post-call observations before checking the selected identity and paths.
pub(super) fn verify<P: RelocationPlatform>(
    platform: &P,
    context: &Context<'_, P>,
    receipt: &mut RelocationReceipt,
    no_op: bool,
) -> Result<(), FileError> {
    let scope = Scope::new(platform, &receipt.request.root)?;
    collect(&scope, receipt)?;
    if scope.root != context.scope.root {
        return Err(changed("root path relocated"));
    }
    check_anchors(platform, context, receipt)?;
    let destination = receipt
        .destination_after
        .as_ref()
        .ok_or_else(|| changed("destination missing after rename"))?;
    let selected = &context.source.info;
    if destination.identity != selected.identity
        || destination.kind != selected.kind
        || destination.data_state != selected.data_state
        || destination.size != selected.size
        || destination.modified != selected.modified
        || destination.mode != selected.mode
        || destination.created != selected.created
        || destination.link_target != selected.link_target
        || platform.stamp(&context.source.file.metadata()?).version != destination.version
    {
        return Err(changed(
            "destination differs from selected source; inspect actual paths, do not retry or rollback",
        ));
    }
    if no_op {
        if destination.version != selected.version {
            return Err(changed("no-op source changed"));
        }
    } else if receipt.source_after.is_some() {
        return Err(changed("source path still occupied after rename"));
    }
    recheck(&scope, receipt)?;
    check_anchors(platform, context, receipt)?;
    scope.revalidate_root()
}
fn check_anchors<P: RelocationPlatform>(
    platform: &P,
    context: &Context<'_, P>,
    receipt: &RelocationReceipt,
) -> Result<(), FileError> {
    for (file, before, after) in [
        (&context.root, &context.root_info, &receipt.root_after),
        (
            &context.source_parent.file,
            &context.source_parent.info,
            &receipt.source_parent_after,
        ),
        (
            &context.destination_parent.file,
            &context.destination_parent.info,
            &receipt.destination_parent_after,
        ),
    ] {
        let stamp = platform.stamp(&file.metadata()?);
        let after = after
            .as_ref()
            .ok_or_else(|| changed("missing parent observation"))?;
        if stamp.identity != before.identity
            || stamp.version != after.version
            || after.identity != before.identity
        {
            return Err(changed(
                "root or parent no longer matches selected descriptor",
            ));
        }
    }
    Ok(())
}
fn recheck<P: RelocationPlatform>(
    scope: &Scope<'_, P>,
    receipt: &RelocationReceipt,
) -> Result<(), FileError> {
    for (path, observed) in [
        (receipt.request.path.as_path(), &receipt.source_after),
        (
            receipt.request.destination.as_path(),
            &receipt.destination_after,
        ),
        (
            validation::parent(&receipt.request.path),
            &receipt.source_parent_after,
        ),
        (
            validation::parent(&receipt.request.destination),
            &receipt.destination_parent_after,
        ),
    ] {
        if super::destination_revision(&optional(scope, path)?)
            != super::destination_revision(observed)
        {
            return Err(changed("post-call observation changed during verification"));
        }
    }
    Ok(())
}
fn info<P: RelocationPlatform>(scope: &Scope<'_, P>, path: &Path) -> Result<FileInfo, FileError> {
    scope.info(&scope.resolve(path, false, true)?)
}
fn optional<P: RelocationPlatform>(
    scope: &Scope<'_, P>,
    path: &Path,
) -> Result<Option<FileInfo>, FileError> {
    match info(scope, path) {
        Ok(info) => Ok(Some(info)),
        Err(error) if error.code == FileErrorCode::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
fn changed(message: &str) -> FileError {
    FileError::new(FileErrorCode::Changed, message)
}

/// Preserve whichever actual observations can be obtained after a failed submission.
pub(super) fn observe(
    platform: &impl RelocationPlatform,
    receipt: &mut RelocationReceipt,
) -> Result<(), FileError> {
    let scope = Scope::new(platform, &receipt.request.root)?;
    collect(&scope, receipt)
}
fn collect<P: RelocationPlatform>(
    scope: &Scope<'_, P>,
    receipt: &mut RelocationReceipt,
) -> Result<(), FileError> {
    receipt.root_after = Some(info(scope, Path::new("."))?);
    receipt.source_after = optional(scope, &receipt.request.path)?;
    receipt.source_path_absent = Some(receipt.source_after.is_none());
    receipt.destination_after = optional(scope, &receipt.request.destination)?;
    receipt.source_parent_after = Some(info(scope, validation::parent(&receipt.request.path))?);
    receipt.destination_parent_after = Some(info(
        scope,
        validation::parent(&receipt.request.destination),
    )?);
    Ok(())
}
