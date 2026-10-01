use super::*;

/// Map one native URL into scope without probing an outside-root file.
pub(super) fn item(request: &FinderRequest, root: &FileInfo, raw: RawItem) -> FinderItem {
    let result = || -> Result<Option<(String, FileInfo)>, FileError> {
        let url = match raw {
            RawItem::Available { url } => url,
            RawItem::Error { error } => return Err(error),
        };
        let path = decode_url(&url)?;
        let relative = path
            .strip_prefix(&root.path)
            .or_else(|_| path.strip_prefix(&request.root))
            .ok();
        let Some(relative) = relative else {
            return Ok(None);
        };
        let relative = if relative.as_os_str().is_empty() {
            Path::new(".")
        } else {
            relative
        };
        let relative_text = relative.to_str().ok_or_else(|| {
            FileError::new(
                FileErrorCode::UnsupportedPathEncoding,
                "Finder path is not UTF-8",
            )
        })?;
        let file = observe(&request.root, relative, false, None)?;
        Ok(Some((relative_text.to_owned(), file)))
    };
    match result() {
        Ok(Some((relative_path, file))) => FinderItem::Available {
            relative_path,
            file: Box::new(file),
        },
        Ok(None) => FinderItem::OutsideScope,
        Err(error) if error.code == FileErrorCode::OutsideRoot => FinderItem::OutsideScope,
        Err(error) => FinderItem::Error { error },
    }
}

fn decode_url(url: &str) -> Result<PathBuf, FileError> {
    if url.len() > 16384 {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "Finder URL exceeds 16 KiB",
        ));
    }
    let path = url
        .strip_prefix("file://localhost/")
        .or_else(|| url.strip_prefix("file:///"))
        .ok_or_else(|| {
            FileError::new(
                FileErrorCode::UnsupportedType,
                "Finder item has no supported local file URL",
            )
        })?;
    if path.contains(['?', '#']) || !valid_escapes(path) {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "Finder file URL has invalid escapes or query/fragment",
        ));
    }
    let path = urlencoding::decode(path).map_err(|_| {
        FileError::new(
            FileErrorCode::UnsupportedPathEncoding,
            "Finder URL path is not UTF-8",
        )
    })?;
    if path.contains('\0') {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "Finder path contains NUL",
        ));
    }
    Ok(PathBuf::from(format!("/{path}")))
}

fn valid_escapes(path: &str) -> bool {
    let bytes = path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    true
}
