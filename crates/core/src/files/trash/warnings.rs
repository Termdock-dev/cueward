use super::*;

pub(super) fn classify(source: &FileInfo, resources: &ResourceMetadata) -> TrashTargetKind {
    match source.kind {
        FileKind::Symlink => TrashTargetKind::Symlink,
        FileKind::Other => TrashTargetKind::SpecialEntry,
        FileKind::File => match resources.is_alias_file {
            ResourceValue::Available { value: true } => TrashTargetKind::FinderAlias,
            ResourceValue::Available { value: false } => TrashTargetKind::RegularFile,
            _ => TrashTargetKind::Unknown,
        },
        FileKind::Directory => match resources.is_package {
            ResourceValue::Available { value: true } => TrashTargetKind::PackageDirectory,
            ResourceValue::Available { value: false } => TrashTargetKind::Directory,
            _ => TrashTargetKind::Unknown,
        },
    }
}
pub(super) fn collect(
    root: &FileInfo,
    source: &FileInfo,
    parent: &FileInfo,
    resources: &ResourceMetadata,
) -> Vec<TrashWarning> {
    let mut warnings = Vec::new();
    match source.kind {
        FileKind::Directory => {
            warnings.push(TrashWarning::WholeDirectoryEntry);
            if !matches!(resources.is_package, ResourceValue::Available { .. }) {
                warnings.push(TrashWarning::PackageStateUnknown);
            }
        }
        FileKind::File => match resources.is_alias_file {
            ResourceValue::Available { value: true } => {
                warnings.push(TrashWarning::AliasFileItself)
            }
            ResourceValue::Available { value: false } => (),
            _ => warnings.push(TrashWarning::AliasStateUnknown),
        },
        FileKind::Symlink => warnings.push(TrashWarning::SymlinkItself),
        FileKind::Other => warnings.push(TrashWarning::SpecialEntry),
    }
    if source.data_state == DataState::Dataless {
        warnings.push(TrashWarning::SourceDataless);
    }
    for (info, warning) in [
        (source, TrashWarning::SourceAvailabilityUnknown),
        (root, TrashWarning::RootAvailabilityUnknown),
        (parent, TrashWarning::ParentAvailabilityUnknown),
    ] {
        if info.data_state == DataState::Unknown {
            warnings.push(warning);
        }
    }
    if source.readonly {
        warnings.push(TrashWarning::SourceReadonlyMode);
    }
    if parent.readonly {
        warnings.push(TrashWarning::ParentReadonlyMode);
    }
    warnings
}
