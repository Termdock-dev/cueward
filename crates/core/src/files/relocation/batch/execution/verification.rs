use super::*;
use crate::files::scope::Scope;
use std::fs::File;
use std::path::Path;

pub(super) fn same(current: &FileInfo, expected: &FileInfo) -> Result<(), FileError> {
    if current.path != expected.path
        || current.identity != expected.identity
        || current.version != expected.version
    {
        return Err(changed("batch path, identity or revision changed"));
    }
    Ok(())
}
pub(super) fn root(
    platform: &impl BatchRenamePlatform,
    held: &File,
    original: &Path,
    expected: &FileInfo,
) -> Result<(), FileError> {
    if original.canonicalize()?.to_str() != Some(&expected.path) {
        return Err(changed("supplied root mapping changed"));
    }
    let stamp = platform.stamp(&held.metadata()?);
    if stamp.identity != expected.identity || stamp.version != expected.version {
        return Err(changed("held root revision changed"));
    }
    let scope = Scope::new(platform, Path::new(&expected.path))?;
    same(
        &scope.info(&scope.resolve(Path::new("."), false, false)?)?,
        expected,
    )
}
pub(super) fn finish(
    platform: &impl BatchRenamePlatform,
    held: &File,
    original: &Path,
    expected: &FileInfo,
    plan: &BatchRenamePlan,
    completed: &[RelocationReceipt],
) -> Result<(), FileError> {
    root(platform, held, original, expected)?;
    let scope = Scope::new(platform, Path::new(&expected.path))?;
    for child in completed {
        completed_child(&scope, child)?;
    }
    for item in &plan.items {
        let before = item
            .proposal
            .as_ref()
            .ok_or_else(|| changed("missing parent anchor"))?;
        let parent = super::super::super::validation::parent(&before.request.path);
        same(
            &scope.info(&scope.resolve(parent, false, false)?)?,
            &before.source_parent,
        )?;
    }
    root(platform, held, original, expected)
}

fn completed_child(
    scope: &Scope<'_, impl BatchRenamePlatform>,
    child: &RelocationReceipt,
) -> Result<(), FileError> {
    let after = child
        .destination_after
        .as_ref()
        .ok_or_else(|| changed("missing verified destination"))?;
    let current = scope.info(&scope.resolve(
        &child.request.destination,
        false,
        child.request.link_itself,
    )?)?;
    same(&current, after)?;
    if child.request.link_itself && current.link_target != after.link_target {
        return Err(changed("completed link reference changed"));
    }
    if child.request.path != child.request.destination {
        match scope.resolve(&child.request.path, false, true) {
            Err(e) if e.code == FileErrorCode::NotFound => (),
            _ => {
                return Err(changed(
                    "a completed source path is occupied or unobservable",
                ));
            }
        }
    }
    Ok(())
}
