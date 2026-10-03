use super::population::Staged;
use super::*;

pub(super) fn bound_stamp(stamp: &FileStamp) -> Result<(), FileError> {
    if serde_json::to_vec(&stamp.identity).map_err(internal)?.len() > 512
        || serde_json::to_vec(&stamp.version).map_err(internal)?.len() > 512
    {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "tree identity/revision exceeds receipt field bounds",
        ));
    }
    Ok(())
}
pub(super) fn bound_initial(receipt: &TreeReceipt) -> Result<(), FileError> {
    let size = serde_json::to_vec_pretty(receipt).map_err(internal)?.len();
    for node in &receipt.nodes {
        bound_stamp(&FileStamp {
            identity: node.source_identity.clone(),
            version: node.source_version.clone(),
            data_state: DataState::NotDataless,
            mode: None,
        })?;
    }
    for info in [
        &receipt.root_before,
        &receipt.parent_before,
        &receipt.source_before,
    ]
    .into_iter()
    .flatten()
    {
        bound_info(info)?;
    }
    if size > 65536 {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "initial tree receipt exceeds 64 KiB; reduce entries or paths",
        ));
    }
    Ok(())
}
fn bound_info(info: &FileInfo) -> Result<(), FileError> {
    bound_stamp(&FileStamp {
        identity: info.identity.clone(),
        version: info.version.clone(),
        data_state: info.data_state.clone(),
        mode: info.mode,
    })?;
    if serde_json::to_vec_pretty(info).map_err(internal)?.len() > 4096 {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "tree observation exceeds 4 KiB receipt bounds",
        ));
    }
    Ok(())
}
fn internal(error: impl std::fmt::Display) -> FileError {
    FileError::new(FileErrorCode::Internal, error.to_string())
}

pub(super) fn verify_private<P: TreeExecutionPlatform>(
    context: &context::Context<'_, P>,
    staged: &Staged,
    receipt: &TreeReceipt,
) -> Result<(), FileError> {
    let scope = Scope::new(context.platform, &staged.storage.path)?;
    verify_tree(context, staged, receipt, &scope, Path::new("."), false)?;
    scope.revalidate_root()
}

pub(super) fn finish<P: TreeExecutionPlatform>(
    context: &context::Context<'_, P>,
    staged: &Staged,
    receipt: &mut TreeReceipt,
) -> Result<(), FileError> {
    context.revalidate(receipt, true)?;
    let scope = context.scope()?;
    let parent_path = receipt
        .request
        .destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = scope.info(&scope.resolve(parent_path, false, false)?)?;
    let observations = verify_tree(
        context,
        staged,
        receipt,
        &scope,
        &receipt.request.destination,
        true,
    )?;
    context.revalidate(receipt, true)?;
    recheck_nodes(context, staged, receipt, &scope, &observations)?;
    let parent_after = scope.info(&scope.resolve(parent_path, false, false)?)?;
    if parent_after.version != parent.version {
        return Err(changed("destination parent changed during verification"));
    }
    scope.revalidate_root()?;
    let root_after = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
    for info in [&root_after, &parent_after, &observations[0]] {
        bound_info(info)?;
    }
    for (node, after) in receipt.nodes.iter_mut().zip(&observations) {
        node.destination_identity = Some(after.identity.clone());
        node.destination_version = Some(after.version.clone());
    }
    receipt.root_after = Some(root_after);
    receipt.parent_after = Some(parent_after);
    receipt.destination_after = Some(observations[0].clone());
    Ok(())
}

fn recheck_nodes<P: TreeExecutionPlatform>(
    context: &context::Context<'_, P>,
    staged: &Staged,
    receipt: &TreeReceipt,
    scope: &Scope<'_, P>,
    observations: &[FileInfo],
) -> Result<(), FileError> {
    for (index, before) in observations.iter().enumerate() {
        let path = mapped(
            &receipt.request.destination,
            &receipt.nodes[index].relative_path,
        );
        let now = scope.info(&scope.resolve(&path, false, true)?)?;
        if now.version != before.version
            || now.identity != before.identity
            || context
                .platform
                .stamp(&staged.files[index].metadata()?)
                .version
                != before.version
        {
            return Err(changed("published node changed during final checks"));
        }
    }
    Ok(())
}
fn verify_tree<P: TreeExecutionPlatform>(
    context: &context::Context<'_, P>,
    staged: &Staged,
    receipt: &TreeReceipt,
    scope: &Scope<'_, P>,
    base: &Path,
    published: bool,
) -> Result<Vec<FileInfo>, FileError> {
    let mut observations = Vec::new();
    for (index, node) in receipt.nodes.iter().enumerate() {
        let path = mapped(base, &node.relative_path);
        let info = scope.info(&scope.resolve(&path, false, true)?)?;
        let file = super::super::traversal::open(context.platform, &info)?;
        let stamp = context.platform.stamp(&staged.files[index].metadata()?);
        if info.kind != node.kind
            || Some(&info.identity) != node.destination_identity.as_ref()
            || stamp.version != info.version
            || (!published || index != 0)
                && Some(&info.version) != node.destination_version.as_ref()
        {
            return Err(changed("created tree path/descriptor/revision changed"));
        }
        verify_metadata(context, &file, index)?;
        match node.kind {
            FileKind::File | FileKind::Symlink => verify_readable(
                context.platform,
                &file,
                &info,
                node.verification
                    .as_ref()
                    .ok_or_else(|| changed("missing file verification"))?,
            )?,
            FileKind::Directory => {
                verify_children(context.platform, &file, receipt, &node.relative_path)?
            }
            _ => return Err(changed("unsupported destination node")),
        }
        if context.platform.stamp(&file.metadata()?).version != info.version {
            return Err(changed("created node changed during read-back"));
        }
        observations.push(info);
    }
    Ok(observations)
}
fn verify_metadata<P: TreeExecutionPlatform>(
    context: &context::Context<'_, P>,
    file: &File,
    index: usize,
) -> Result<(), FileError> {
    let source = context.sources[index].metadata()?;
    let actual = file.metadata()?;
    if actual.permissions() != source.permissions()
        || actual.modified()? != source.modified()?
        || context.platform.tree_attribute_digest(file)? != context.attributes[index]
    {
        return Err(FileError::new(
            FileErrorCode::VerificationFailed,
            "created node permissions/mtime/xattrs differ",
        ));
    }
    Ok(())
}
fn verify_children(
    platform: &impl TreeExecutionPlatform,
    file: &File,
    receipt: &TreeReceipt,
    relative: &Path,
) -> Result<(), FileError> {
    let mut names = platform.tree_names(file, MAX_EXECUTION_ENTRIES)?;
    names.sort();
    let mut expected: Vec<_> = receipt
        .nodes
        .iter()
        .skip(1)
        .filter(|node| {
            node.relative_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                == relative
        })
        .filter_map(|node| {
            node.relative_path
                .file_name()
                .map(std::ffi::OsStr::to_owned)
        })
        .collect();
    expected.sort();
    if names != expected {
        return Err(changed(
            "created directory entries differ from selected tree",
        ));
    }
    Ok(())
}
fn mapped(base: &Path, relative: &Path) -> PathBuf {
    if relative == Path::new(".") {
        base.to_owned()
    } else {
        base.join(relative)
    }
}
