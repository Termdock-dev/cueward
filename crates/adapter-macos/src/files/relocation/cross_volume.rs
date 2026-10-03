//! Reuse verified object copying; retain the original rather than unlinking it after publication.
use super::*;
use crate::files::mutation::{
    self, MutationAction, MutationReceipt, MutationRequest, MutationStatus,
};
use cueward_core::files::mutation::verify_readable;
use cueward_core::files::trash::execution::TrashPlatform;
use std::path::PathBuf;
const COPIES: Store = Store("files");

pub(super) fn execute(
    receipt: &mut RelocationReceipt,
    checkpoint: &mut dyn FnMut(&RelocationReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    if receipt.request.action != RelocationAction::Move {
        return Err(changed("cross-volume rename is not a sibling operation"));
    }
    let mut child = create_copy(receipt)?;
    receipt.copy_operation_id = Some(child.operation_id.clone());
    receipt.copied = None;
    receipt.renamed = Some(false);
    receipt.stage = RelocationStage::Prepared;
    checkpoint(receipt)?;
    COPIES.claim(&child.operation_id)?;
    receipt.mutation_attempted = true;
    checkpoint(receipt)?;
    if let Err(error) = cueward_core::files::mutation::execute(&MacFiles, &mut child, &mut |r| {
        COPIES.save(&r.operation_id, &r.receipt_path, r)
    }) {
        cueward_core::files::mutation::record_error(&mut child, error);
    }
    COPIES.save(&child.operation_id, &child.receipt_path, &child)?;
    receipt.copied = if child.status == MutationStatus::NotStarted {
        Some(false)
    } else {
        child.destination_created
    };
    receipt.destination_after = child.destination_after.clone();
    checkpoint(receipt)?;
    if child.status != MutationStatus::Completed || !child.completion_verified {
        return Err(child
            .error
            .take()
            .unwrap_or_else(|| changed("cross-volume copy did not complete")));
    }
    retain_source(receipt, &child, checkpoint)?;
    finish(receipt, &child)?;
    receipt.status = RelocationStatus::Completed;
    receipt.stage = RelocationStage::Finished;
    receipt.completion_verified = true;
    checkpoint(receipt)
}
fn create_copy(receipt: &RelocationReceipt) -> Result<MutationReceipt, FileError> {
    let request = MutationRequest {
        root: receipt.request.root.clone(),
        destination: receipt.request.destination.clone(),
        expected_parent_version: receipt.request.expected_parent_version.clone(),
        action: MutationAction::Copy {
            path: receipt.request.path.clone(),
            expected_version: receipt.request.expected_version.clone(),
            max_bytes: 268435456,
        },
    };
    COPIES.create(|id, path| MutationReceipt::new(id, request, path))
}

fn retain_source(
    receipt: &mut RelocationReceipt,
    child: &MutationReceipt,
    checkpoint: &mut dyn FnMut(&RelocationReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    let root = MutationPlatform::open_directory(
        &MacFiles,
        Path::new(
            &receipt
                .before
                .as_ref()
                .ok_or_else(|| changed("missing plan"))?
                .root
                .path,
        ),
    )?;
    check_root(receipt, &root)?;
    let (storage, path) = retain_storage(receipt, child, &root)?;
    receipt.retained_source_path = Some(
        path.to_str()
            .ok_or_else(|| changed("non-UTF-8 backup path"))?
            .into(),
    );
    receipt.source_quarantined = None;
    receipt.stage = RelocationStage::Renaming;
    checkpoint(receipt)?;
    check_root(receipt, &root)?;
    let source = selected_source(receipt, child)?;
    let moved = MacFiles.stage_source(
        &root,
        &receipt.request.path,
        &storage.directory,
        Path::new("retained-source"),
    );
    receipt.source_quarantined = moved.destination_created;
    checkpoint(receipt)?;
    moved.result?;
    receipt.source_after = Some(verify_retained(&path, &source, child)?);
    Ok(())
}
fn retain_storage(
    receipt: &RelocationReceipt,
    child: &MutationReceipt,
    root: &File,
) -> Result<(mutation::Staging, PathBuf), FileError> {
    let parent_path = receipt
        .request
        .path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = crate::files::observe(&receipt.request.root, parent_path, false, None)?;
    let proxy = MutationReceipt::new(
        child.operation_id.clone(),
        MutationRequest {
            root: receipt.request.root.clone(),
            destination: receipt.request.path.clone(),
            expected_parent_version: parent.version.clone(),
            action: MutationAction::Mkdir,
        },
        child.receipt_path.clone(),
    );
    let storage = MacFiles.prepare_staging(&proxy, root)?;
    let path = storage
        .path
        .parent()
        .ok_or_else(|| changed("missing backup parent"))?
        .join("retained-source");
    if path.starts_with(
        &receipt
            .before
            .as_ref()
            .ok_or_else(|| changed("missing plan"))?
            .source
            .path,
    ) {
        return Err(changed("retained original must not be inside source tree"));
    }
    Ok((storage, path))
}
fn selected_source(
    receipt: &RelocationReceipt,
    child: &MutationReceipt,
) -> Result<FileInfo, FileError> {
    let before = receipt
        .before
        .as_ref()
        .ok_or_else(|| changed("missing plan"))?;
    let parent_path = receipt
        .request
        .path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let source = crate::files::observe(
        &receipt.request.root,
        &receipt.request.path,
        false,
        Some(receipt.request.expected_version.clone()),
    )?;
    let current_parent = crate::files::observe(&receipt.request.root, parent_path, false, None)?;
    if current_parent.identity != before.source_parent.identity {
        return Err(changed("source parent replaced before quarantine"));
    }
    let held = MacFiles.open_object(&source)?;
    verify_readable(&MacFiles, &held, &source, proof(child)?)?;
    verify_destination(child)?;
    Ok(source)
}
fn verify_retained(
    path: &Path,
    source: &FileInfo,
    child: &MutationReceipt,
) -> Result<FileInfo, FileError> {
    let retained = observe(path)?;
    if retained.identity != source.identity
        || retained.mode != source.mode
        || retained.modified != source.modified
        || retained.kind != source.kind
    {
        return Err(changed(
            "quarantined object differs from original selection",
        ));
    }
    let held = MacFiles.open_object(&retained)?;
    verify_readable(&MacFiles, &held, &retained, proof(child)?)?;
    if MacFiles.read_copy_attributes(&held)?
        != (
            proof(child)?.extended_attributes_sha256.clone(),
            proof(child)?.extended_attributes_bytes,
        )
    {
        return Err(changed(
            "retained source attributes differ from copied proof",
        ));
    }
    Ok(retained)
}
fn finish(receipt: &mut RelocationReceipt, child: &MutationReceipt) -> Result<(), FileError> {
    let before = receipt
        .before
        .as_ref()
        .ok_or_else(|| changed("missing initial plan"))?;
    let root = MutationPlatform::open_directory(&MacFiles, Path::new(&before.root.path))?;
    check_root(receipt, &root)?;
    match crate::files::observe(&receipt.request.root, &receipt.request.path, false, None) {
        Err(e) if e.code == FileErrorCode::NotFound => receipt.source_path_absent = Some(true),
        _ => return Err(changed("source pathname occupied after quarantine")),
    }
    verify_destination(child)?;
    let source_parent = receipt
        .request
        .path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let dest_parent = receipt
        .request
        .destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let a = crate::files::observe(&receipt.request.root, source_parent, false, None)?;
    let b = crate::files::observe(&receipt.request.root, dest_parent, false, None)?;
    if a.identity != before.source_parent.identity
        || b.identity != before.destination_parent.identity
    {
        return Err(changed("move parent identity changed"));
    }
    receipt.source_parent_after = Some(a);
    receipt.destination_parent_after = Some(b);
    receipt.root_after = Some(crate::files::observe(
        &receipt.request.root,
        Path::new("."),
        false,
        None,
    )?);
    Ok(())
}
fn check_root(receipt: &RelocationReceipt, held: &File) -> Result<(), FileError> {
    let before = receipt
        .before
        .as_ref()
        .ok_or_else(|| changed("missing root plan"))?;
    let now = crate::files::observe(&receipt.request.root, Path::new("."), false, None)?;
    if now.path != before.root.path
        || now.identity != before.root.identity
        || MacFiles.stamp(&held.metadata()?).version != now.version
    {
        return Err(changed("move root mapping changed"));
    }
    Ok(())
}
fn verify_destination(child: &MutationReceipt) -> Result<(), FileError> {
    let expected = child
        .destination_after
        .as_ref()
        .ok_or_else(|| changed("missing copied destination"))?;
    let info = crate::files::observe(
        &child.request.root,
        &child.request.destination,
        false,
        Some(expected.version.clone()),
    )?;
    let file = MacFiles.open_object(&info)?;
    verify_readable(&MacFiles, &file, &info, proof(child)?)?;
    if MacFiles.read_copy_attributes(&file)?
        != (
            proof(child)?.extended_attributes_sha256.clone(),
            proof(child)?.extended_attributes_bytes,
        )
    {
        return Err(changed("copied destination attributes changed"));
    }
    Ok(())
}
fn proof(child: &MutationReceipt) -> Result<&mutation::CopyVerification, FileError> {
    child
        .verification
        .as_ref()
        .ok_or_else(|| changed("copy proof missing"))
}
fn observe(path: &Path) -> Result<FileInfo, FileError> {
    crate::files::observe(
        path.parent().ok_or_else(|| changed("missing parent"))?,
        Path::new(path.file_name().ok_or_else(|| changed("missing leaf"))?),
        false,
        None,
    )
}
fn changed(message: &str) -> FileError {
    FileError::new(FileErrorCode::Changed, message)
}
