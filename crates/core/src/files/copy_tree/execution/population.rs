use super::*;

pub(super) struct Staged {
    pub storage: Staging,
    pub files: Vec<File>,
}
pub(super) fn prepare<P: TreeExecutionPlatform>(
    context: &context::Context<'_, P>,
    receipt: &mut TreeReceipt,
    checkpoint: &mut impl FnMut(&TreeReceipt) -> Result<(), FileError>,
) -> Result<Staged, FileError> {
    let proxy = MutationReceipt::new(
        receipt.operation_id.clone(),
        MutationRequest {
            root: receipt.request.root.clone(),
            destination: receipt.request.destination.clone(),
            expected_parent_version: receipt.request.expected_parent_version.clone(),
            action: MutationAction::Mkdir,
        },
        receipt.receipt_path.clone(),
    );
    let storage = context.platform.prepare_staging(&proxy, &context.root)?;
    let source = Path::new(&context.plan.entries[0].source.path).canonicalize()?;
    if storage.directory.metadata()?.is_dir()
        && storage
            .path
            .parent()
            .ok_or_else(|| changed("missing staging parent"))?
            .canonicalize()?
            .starts_with(&source)
    {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "private receipt storage must not be inside the selected source tree",
        ));
    }
    receipt.staging_path = Some(text(&storage.path)?);
    receipt.stage = TreeStage::Staging;
    checkpoint(receipt)?;
    Ok(Staged {
        storage,
        files: Vec::new(),
    })
}

pub(super) fn populate<P: TreeExecutionPlatform>(
    context: &context::Context<'_, P>,
    staged: &mut Staged,
    receipt: &mut TreeReceipt,
    checkpoint: &mut impl FnMut(&TreeReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    // Construction retains descriptors separately from the storage publication handle.
    let mut files = Vec::new();
    for (index, entry) in context.plan.entries.iter().enumerate() {
        context.check_node(receipt, index)?;
        receipt.active_index = Some(index);
        receipt.stage = TreeStage::Copying;
        checkpoint(receipt)?;
        let mut file = create(context, staged, &files, index)?;
        if entry.source.kind == FileKind::File {
            let verified = copy_file_verified(
                context.platform,
                &context.sources[index],
                &entry.source,
                &mut file,
                entry.source.size,
            )?;
            if (
                verified.extended_attributes_sha256.clone(),
                verified.extended_attributes_bytes,
            ) != context.attributes[index]
            {
                return Err(changed("source attributes changed during copying"));
            }
            receipt.nodes[index].verification = Some(verified);
        }
        files.push(file);
        checkpoint(receipt)?;
    }
    finish_metadata(context, &files, receipt, checkpoint)?;
    staged.files = files;
    // The caller reopens and verifies paths from the private tree; no cached path writes follow.
    Ok(())
}
fn finish_metadata<P: TreeExecutionPlatform>(
    context: &context::Context<'_, P>,
    files: &[File],
    receipt: &mut TreeReceipt,
    checkpoint: &mut impl FnMut(&TreeReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    for index in (0..files.len()).rev() {
        if context.plan.entries[index].source.kind == FileKind::Directory {
            receipt.active_index = Some(index);
            receipt.stage = TreeStage::Metadata;
            checkpoint(receipt)?;
            context.check_node(receipt, index)?;
            directory_metadata(context, &files[index], index)?;
        }
        let stamp = context.platform.stamp(&files[index].metadata()?);
        super::verification::bound_stamp(&stamp)?;
        receipt.nodes[index].destination_identity = Some(stamp.identity);
        receipt.nodes[index].destination_version = Some(stamp.version);
        checkpoint(receipt)?;
    }
    Ok(())
}
fn create<P: TreeExecutionPlatform>(
    context: &context::Context<'_, P>,
    staged: &Staged,
    files: &[File],
    index: usize,
) -> Result<File, FileError> {
    let entry = &context.plan.entries[index];
    let (parent, name) = if index == 0 {
        (&staged.storage.directory, staged.storage.path.file_name())
    } else {
        let parent_path = entry
            .relative_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent_index = context.plan.entries[..index]
            .iter()
            .position(|e| e.relative_path == parent_path)
            .ok_or_else(|| changed("missing staged parent"))?;
        (&files[parent_index], entry.relative_path.file_name())
    };
    let name = name.ok_or_else(|| changed("missing staged leaf"))?;
    match entry.source.kind {
        FileKind::Directory => context.platform.create_directory(parent, name).file,
        FileKind::File => context.platform.create_file(parent, name).file,
        _ => Err(changed("unsupported node reached tree construction")),
    }
}
fn directory_metadata<P: TreeExecutionPlatform>(
    context: &context::Context<'_, P>,
    file: &File,
    index: usize,
) -> Result<(), FileError> {
    let source = &context.sources[index];
    if context.platform.copy_attributes(source, file)? != context.attributes[index] {
        return Err(changed("directory attributes changed during copying"));
    }
    let metadata = source.metadata()?;
    file.set_permissions(metadata.permissions())?;
    file.set_times(std::fs::FileTimes::new().set_modified(metadata.modified()?))?;
    file.sync_all()?;
    Ok(())
}
