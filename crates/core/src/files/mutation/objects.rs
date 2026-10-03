//! Shared directory/link copying for normal transfers, retained trash backups and restoration.
use super::*;
use sha2::{Digest, Sha256};

pub(super) fn copy(
    platform: &impl MutationPlatform,
    source: &File,
    info: &FileInfo,
    destination: &mut File,
    maximum: u64,
) -> Result<CopyVerification, FileError> {
    copy_node(platform, source, info, destination, maximum, 0, &mut 0)
}
fn copy_node(
    platform: &impl MutationPlatform,
    source: &File,
    info: &FileInfo,
    destination: &mut File,
    maximum: u64,
    depth: usize,
    count: &mut usize,
) -> Result<CopyVerification, FileError> {
    *count += 1;
    if depth > 32 || *count > 256 {
        return Err(limit());
    }
    context::check_source(platform, source, info)?;
    platform.validate_copy_source(source, info)?;
    if info.kind == FileKind::File {
        return copy_file_verified(platform, source, info, destination, maximum);
    }
    let children = if info.kind == FileKind::Directory {
        Some(copy_children(
            platform,
            source,
            info,
            destination,
            maximum,
            depth,
            count,
        )?)
    } else {
        None
    };
    let link_target = info.link_target.clone();
    let bytes = match &children {
        Some(nodes) => nodes.iter().map(|n| n.verification.bytes).sum(),
        None => link_target
            .as_ref()
            .ok_or_else(|| context::changed("missing link reference"))?
            .len() as u64,
    };
    if bytes > maximum {
        return Err(limit());
    }
    let proof = copy_metadata(platform, source, destination, bytes, children, link_target)?;
    context::check_source(platform, source, info)?;
    verify(platform, source, info, &proof)?;
    Ok(proof)
}
fn copy_metadata(
    platform: &impl MutationPlatform,
    source: &File,
    destination: &File,
    bytes: u64,
    children: Option<Vec<CopyNode>>,
    link_target: Option<String>,
) -> Result<CopyVerification, FileError> {
    let content_sha256 = digest(&children, &link_target)?;
    let (attributes, attribute_bytes) = platform.copy_attributes(source, destination)?;
    let metadata = source.metadata()?;
    destination.set_permissions(metadata.permissions())?;
    destination.set_times(std::fs::FileTimes::new().set_modified(metadata.modified()?))?;
    if metadata.is_dir() {
        destination.sync_all()?;
    }
    let actual = destination.metadata()?;
    Ok(CopyVerification {
        content_sha256,
        bytes,
        permissions_equal: actual.permissions() == metadata.permissions(),
        modified_equal: actual.modified()? == metadata.modified()?,
        extended_attributes_sha256: attributes,
        extended_attributes_bytes: attribute_bytes,
        link_target,
        children,
    })
}

fn copy_children(
    platform: &impl MutationPlatform,
    source: &File,
    info: &FileInfo,
    destination: &File,
    maximum: u64,
    depth: usize,
    count: &mut usize,
) -> Result<Vec<CopyNode>, FileError> {
    let selected = platform.copy_children(info, source, 256 - *count)?;
    let mut children = Vec::new();
    let mut bytes = 0;
    for (child, held) in selected {
        let mut created = platform
            .create_object(destination, OsStr::new(&child.name), &child)
            .file?;
        let verification = copy_node(
            platform,
            &held,
            &child,
            &mut created,
            maximum - bytes,
            depth + 1,
            count,
        )?;
        bytes += verification.bytes;
        children.push(CopyNode {
            name: child.name,
            kind: child.kind,
            mode: child.mode,
            modified: child.modified,
            verification,
        });
    }
    Ok(children)
}
pub(super) fn verify(
    platform: &impl MutationPlatform,
    file: &File,
    info: &FileInfo,
    proof: &CopyVerification,
) -> Result<(), FileError> {
    context::check_source(platform, file, info)?;
    if !proof.permissions_equal || !proof.modified_equal {
        return Err(context::changed(
            "copied object attributes differ from proof",
        ));
    }
    if info.kind == FileKind::Symlink {
        if info.link_target != proof.link_target
            || digest(&None, &info.link_target)? != proof.content_sha256
        {
            return Err(context::changed("copied symlink reference changed"));
        }
    } else {
        verify_children(platform, file, info, proof)?;
    }
    context::check_source(platform, file, info)
}
fn verify_children(
    platform: &impl MutationPlatform,
    file: &File,
    info: &FileInfo,
    proof: &CopyVerification,
) -> Result<(), FileError> {
    let expected = proof
        .children
        .as_ref()
        .ok_or_else(|| context::changed("missing directory proof"))?;
    let actual = platform.copy_children(info, file, expected.len())?;
    if actual.len() != expected.len() {
        return Err(context::changed("copied child set differs"));
    }
    for ((child, held), node) in actual.iter().zip(expected) {
        if child.name != node.name
            || child.kind != node.kind
            || child.mode != node.mode
            || child.modified != node.modified
        {
            return Err(context::changed("copied child metadata differs"));
        }
        verify_readable(platform, held, child, &node.verification)?;
        if platform.read_copy_attributes(held)?
            != (
                node.verification.extended_attributes_sha256.clone(),
                node.verification.extended_attributes_bytes,
            )
        {
            return Err(context::changed("copied child attributes differ"));
        }
    }
    if digest(&proof.children, &None)? != proof.content_sha256 {
        return Err(context::changed("invalid directory proof"));
    }
    Ok(())
}

fn digest(children: &Option<Vec<CopyNode>>, target: &Option<String>) -> Result<String, FileError> {
    let bytes = if let Some(target) = target {
        target.as_bytes().to_vec()
    } else {
        serde_json::to_vec(children)
            .map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))?
    };
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn limit() -> FileError {
    FileError::new(
        FileErrorCode::ScanLimit,
        "copy exceeds 256 nodes, depth 32 or max-bytes",
    )
}

pub(super) fn children(
    platform: &impl MutationPlatform,
    info: &FileInfo,
    file: &File,
    maximum: usize,
) -> Result<Vec<(FileInfo, File)>, FileError> {
    context::check_source(platform, file, info)?;
    let scope = crate::files::scope::Scope::new(platform, Path::new(&info.path))?;
    if scope.root != Path::new(&info.path) {
        return Err(context::changed("directory path retargeted"));
    }
    let mut names = platform.copy_names(file, maximum)?;
    names.sort();
    let mut result = Vec::new();
    for name in names {
        let child = scope.info(&scope.resolve(Path::new(&name), false, true)?)?;
        let held = platform.open_object(&child)?;
        context::check_source(platform, &held, &child)?;
        result.push((child, held));
    }
    scope.revalidate_root()?;
    context::check_source(platform, file, info)?;
    Ok(result)
}

/// Advance only anchors affected by allocation of this private staging directory.
pub(crate) fn advance_staging(
    platform: &impl MutationPlatform,
    storage: &Staging,
    root_path: &Path,
    root: &File,
    parent: &File,
    root_info: &mut FileInfo,
    parent_info: &mut FileInfo,
) -> Result<(), FileError> {
    if storage.path.parent().and_then(Path::parent) != Some(Path::new(&parent_info.path)) {
        return Ok(());
    }
    let scope = crate::files::scope::Scope::new(platform, root_path)?;
    let now_root = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
    let relative = Path::new(&parent_info.path)
        .strip_prefix(&scope.root)
        .map_err(|_| context::changed("staging parent escaped root"))?;
    let now_parent = scope.info(&scope.resolve(
        if relative.as_os_str().is_empty() {
            Path::new(".")
        } else {
            relative
        },
        false,
        false,
    )?)?;
    if now_root.path != root_info.path
        || now_root.identity != root_info.identity
        || now_parent.path != parent_info.path
        || now_parent.identity != parent_info.identity
        || platform.stamp(&root.metadata()?).version != now_root.version
        || platform.stamp(&parent.metadata()?).version != now_parent.version
    {
        return Err(context::changed("staging anchor moved or replaced"));
    }
    *root_info = now_root;
    *parent_info = now_parent;
    scope.revalidate_root()
}
