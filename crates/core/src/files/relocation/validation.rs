use super::*;
use crate::files::scope::text;
use std::path::Component;

/// Construct a same-parent rename without accepting a path as the new name.
pub fn rename_destination(path: &Path, name: &Path) -> Result<PathBuf, FileError> {
    normal(path)?;
    normal(name)?;
    if name.components().count() != 1 || text(name)?.contains('/') {
        return Err(invalid(
            "name must be one leaf component, not a destination path",
        ));
    }
    Ok(path.parent().unwrap_or(Path::new("")).join(name))
}

/// Require explicit relative paths and revision observations.
pub(super) fn validate(request: &RelocationRequest) -> Result<(), FileError> {
    normal(&request.path)?;
    normal(&request.destination)?;
    if request.expected_version.is_empty() || request.expected_parent_version.is_empty() {
        return Err(invalid(
            "expected-version and expected-parent-version are required",
        ));
    }
    if request.action == RelocationAction::Rename
        && parent(&request.path) != parent(&request.destination)
    {
        return Err(invalid(
            "rename must keep the source parent; use a move plan for another parent",
        ));
    }
    Ok(())
}

fn normal(path: &Path) -> Result<(), FileError> {
    let value = text(path)?;
    if value.contains('\0')
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(invalid(
            "source and destination must contain only relative normal components",
        ));
    }
    Ok(())
}

/// Resolve an empty relative parent as the selected root for observation.
pub(super) fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}
fn invalid(message: &str) -> FileError {
    FileError::new(FileErrorCode::InvalidOptions, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rename_accepts_exact_unicode_leaf_not_another_path() {
        assert_eq!(
            rename_destination(Path::new("owned"), Path::new("new")).unwrap(),
            PathBuf::from("new")
        );
        assert_eq!(
            rename_destination(Path::new("from/owned"), Path::new("臺灣\n<external>")).unwrap(),
            PathBuf::from("from/臺灣\n<external>")
        );
        for name in [
            "",
            ".",
            "..",
            "../outside",
            "/absolute",
            "other/path",
            "name/",
            "NUL\0name",
        ] {
            assert_eq!(
                rename_destination(Path::new("owned"), Path::new(name))
                    .unwrap_err()
                    .code,
                FileErrorCode::InvalidOptions
            );
        }
    }
    #[test]
    fn request_requires_relative_selection_revisions_and_same_parent_for_rename() {
        let mut request = RelocationRequest {
            root: "/owned".into(),
            path: "from/source".into(),
            destination: "from/destination".into(),
            expected_version: "source".into(),
            expected_parent_version: "parent".into(),
            action: RelocationAction::Rename,
        };
        assert!(validate(&request).is_ok());
        request.destination = "to/destination".into();
        assert!(validate(&request).is_err());
        request.action = RelocationAction::Move;
        assert!(validate(&request).is_ok());
        for path in [
            ".",
            "..",
            "/absolute",
            "../outside",
            "a/../outside",
            "NUL\0path",
        ] {
            request.path = path.into();
            assert!(validate(&request).is_err());
        }
        request.path = "owned".into();
        request.expected_version.clear();
        assert!(validate(&request).is_err());
        request.expected_version = "source".into();
        request.expected_parent_version.clear();
        assert!(validate(&request).is_err());
    }
}
