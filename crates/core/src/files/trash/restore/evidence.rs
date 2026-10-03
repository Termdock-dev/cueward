use super::*;

pub(super) struct Evidence<'a> {
    pub root: &'a FileInfo,
    pub parent: &'a FileInfo,
    pub source: &'a FileInfo,
    pub backup: &'a FileInfo,
    pub proof: &'a CopyVerification,
}
/// Require complete current-schema trash evidence with consistent original and backup paths.
pub(super) fn validate<'a>(
    request: &RestoreRequest,
    original: &'a TrashReceipt,
) -> Result<Evidence<'a>, FileError> {
    if original.operation != "trash"
        || original.operation_id != request.trash_operation_id
        || original.status != MutationStatus::Completed
        || original.stage != TrashStage::Finished
        || !original.completion_verified
        || !original.backup_verified
        || !original.mutation_attempted
        || original.source_removed != Some(true)
        || !original.trash_attempted
        || original.moved_to_trash != Some(true)
        || !original.source_absent
        || !original.staged_source_absent
        || original.error.is_some()
        || original.preflight_observations_omitted
        || original.trash_after.is_none()
        || original.trash_attributes.is_none()
    {
        return Err(invalid(
            "restore accepts only a completed verified trash receipt",
        ));
    }
    super::super::execution::validate_request(&original.request)?;
    let missing = || invalid("trash receipt lacks required original/backup evidence");
    let result = Evidence {
        root: original.root_before.as_ref().ok_or_else(missing)?,
        parent: original.parent_before.as_ref().ok_or_else(missing)?,
        source: original.source_before.as_ref().ok_or_else(missing)?,
        backup: original.backup_after.as_ref().ok_or_else(missing)?,
        proof: original.backup_verification.as_ref().ok_or_else(missing)?,
    };
    validate_paths(original, &result)?;
    validate_copy(&result)?;
    if result.proof.bytes > original.request.max_bytes {
        return Err(invalid("backup proof exceeds original transfer bound"));
    }
    Ok(result)
}
fn validate_paths(original: &TrashReceipt, evidence: &Evidence<'_>) -> Result<(), FileError> {
    let store = Path::new(&original.receipt_path)
        .parent()
        .ok_or_else(|| invalid("missing original store"))?;
    let root = Path::new(&evidence.root.path);
    let parent = original
        .request
        .path
        .parent()
        .filter(|p| !p.as_os_str().is_empty());
    let parent = parent.map_or_else(|| root.to_owned(), |p| root.join(p));
    if !root.is_absolute()
        || store.file_name().and_then(|s| s.to_str()) != Some(&original.operation_id)
        || Path::new(&evidence.source.path) != root.join(&original.request.path)
        || Path::new(&evidence.parent.path) != parent
        || (Path::new(&evidence.backup.path) != store.join("staged-object")
            && Path::new(&evidence.backup.path)
                .parent()
                .and_then(Path::file_name)
                .and_then(|n| n.to_str())
                != Some(format!(".cueward-stage-{}", original.operation_id).as_str()))
        || original.backup_path.as_deref() != Some(&evidence.backup.path)
        || evidence.root.kind != FileKind::Directory
        || evidence.parent.kind != FileKind::Directory
    {
        return Err(invalid(
            "trash receipt path/scope relationships are inconsistent",
        ));
    }
    Ok(())
}
fn validate_copy(e: &Evidence<'_>) -> Result<(), FileError> {
    if !matches!(
        e.source.kind,
        FileKind::File | FileKind::Directory | FileKind::Symlink
    ) || e.backup.kind != e.source.kind
        || e.source.data_state != DataState::NotDataless
        || e.backup.data_state != DataState::NotDataless
        || e.source.identity == e.backup.identity
        || (e.source.kind != FileKind::Directory
            && (e.source.size != e.proof.bytes || e.backup.size != e.proof.bytes))
        || (e.source.kind == FileKind::Directory && e.proof.children.is_none())
        || (e.source.kind == FileKind::Symlink
            && (e.proof.link_target != e.source.link_target
                || e.backup.link_target != e.source.link_target))
        || e.source.mode != e.backup.mode
        || e.source.modified != e.backup.modified
        || !e.proof.permissions_equal
        || !e.proof.modified_equal
        || !digest(&e.proof.content_sha256)
        || !digest(&e.proof.extended_attributes_sha256)
        || e.proof.extended_attributes_bytes > 4 * 1024 * 1024
    {
        return Err(invalid(
            "trash backup does not establish independent supported copy evidence",
        ));
    }
    Ok(())
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
