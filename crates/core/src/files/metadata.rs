use super::scope::{Resolved, Scope};
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataError {
    pub domain: String,
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ResourceValue<T> {
    Available { value: T },
    Unavailable,
    NotApplicable,
    Unsupported,
    Error { error: MetadataError },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceMetadata {
    pub content_type: ResourceValue<String>,
    pub finder_tags: ResourceValue<Vec<String>>,
    pub is_package: ResourceValue<bool>,
    pub is_alias_file: ResourceValue<bool>,
}

impl ResourceMetadata {
    /// A platform without resource metadata must not invent empty/false values.
    pub fn unsupported() -> Self {
        Self {
            content_type: ResourceValue::Unsupported,
            finder_tags: ResourceValue::Unsupported,
            is_package: ResourceValue::Unsupported,
            is_alias_file: ResourceValue::Unsupported,
        }
    }

    fn unavailable() -> Self {
        Self {
            content_type: ResourceValue::Unavailable,
            finder_tags: ResourceValue::Unavailable,
            is_package: ResourceValue::Unavailable,
            is_alias_file: ResourceValue::Unavailable,
        }
    }

    fn not_applicable() -> Self {
        Self {
            content_type: ResourceValue::NotApplicable,
            finder_tags: ResourceValue::NotApplicable,
            is_package: ResourceValue::NotApplicable,
            is_alias_file: ResourceValue::NotApplicable,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMetadata {
    pub file: FileInfo,
    pub resources: ResourceMetadata,
}

pub(super) fn inspect(
    scope: &Scope<'_, impl FilePlatform>,
    resolved: &Resolved,
    file: FileInfo,
) -> Result<FileMetadata, FileError> {
    let resources = if matches!(file.kind, FileKind::Symlink | FileKind::Other) {
        ResourceMetadata::not_applicable()
    } else if file.data_state == DataState::Dataless {
        ResourceMetadata::unavailable()
    } else {
        scope
            .platform
            .resource_metadata(&resolved.path, &resolved.metadata)?
    };
    Ok(FileMetadata { file, resources })
}
