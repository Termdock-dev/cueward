use super::*;

/// Construct a same-parent duplicate destination using one exact new leaf name.
pub fn duplicate_destination(source: &Path, name: &Path) -> Result<PathBuf, FileError> {
    crate::files::relocation::rename_destination(source, name)
}

/// Enforce the sibling-only contract even for prepared requests that bypass CLI parsing.
pub(super) fn validate_sibling(request: &MutationRequest) -> Result<(), FileError> {
    let MutationAction::Duplicate { path, .. } = &request.action else {
        return Ok(());
    };
    let name = request
        .destination
        .file_name()
        .ok_or_else(|| invalid("missing duplicate filename"))?;
    if duplicate_destination(path, Path::new(name))? != request.destination {
        return Err(invalid(
            "duplicate must keep the source parent; use copy for another directory",
        ));
    }
    Ok(())
}
fn invalid(message: &str) -> FileError {
    FileError::new(FileErrorCode::InvalidOptions, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_destination_preserves_exact_leaf_and_parent() {
        assert_eq!(
            duplicate_destination(Path::new("owned"), Path::new("new")).unwrap(),
            Path::new("new")
        );
        assert_eq!(
            duplicate_destination(Path::new("from/owned"), Path::new("臺灣\n<external>")).unwrap(),
            Path::new("from/臺灣\n<external>")
        );
        for name in [
            "",
            ".",
            "..",
            "/absolute",
            "../outside",
            "other/name",
            "name/",
            "NUL\0name",
        ] {
            assert_eq!(
                duplicate_destination(Path::new("owned"), Path::new(name))
                    .unwrap_err()
                    .code,
                FileErrorCode::InvalidOptions
            );
        }
    }
    #[test]
    fn duplicate_request_cannot_choose_another_parent() {
        let mut request = MutationRequest {
            root: "/owned".into(),
            destination: "to/new".into(),
            expected_parent_version: "parent".into(),
            action: MutationAction::Duplicate {
                path: "from/source".into(),
                expected_version: "source".into(),
                max_bytes: 1024,
            },
        };
        assert_eq!(
            validate_sibling(&request).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
        request.destination = "from/new".into();
        assert!(validate_sibling(&request).is_ok());
        request.action = MutationAction::Copy {
            path: "from/source".into(),
            expected_version: "source".into(),
            max_bytes: 1024,
        };
        request.destination = "to/new".into();
        assert!(validate_sibling(&request).is_ok());
    }
}
