//! Bounded name-only tag edits preserve all unselected names, order and colors.
use super::{FileError, FileErrorCode, FileInfo};
use serde::{Deserialize, Serialize};

/// A tag's stored color is observational; this slice cannot edit colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTag {
    pub name: String,
    pub color: Option<u8>,
}

/// Exact stored-attribute revision is independent of file content revisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSnapshot {
    pub file: FileInfo,
    pub tags: Vec<FileTag>,
    pub tags_version: String,
}

/// Add missing names or remove all matching names, without replacing the tag set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", content = "names", rename_all = "snake_case")]
pub enum TagEdit {
    Add(Vec<String>),
    Remove(Vec<String>),
}

/// Produce a bounded edit while preserving unrelated tags and existing colors.
pub fn edit_tags(before: &[FileTag], edit: &TagEdit) -> Result<Vec<FileTag>, FileError> {
    let names = match edit {
        TagEdit::Add(names) | TagEdit::Remove(names) => names,
    };
    if names.is_empty()
        || names.len() > 256
        || names.iter().map(String::len).sum::<usize>() > 8192
        || names
            .iter()
            .any(|n| n.is_empty() || n.len() > 1024 || n.contains(['\0', '\n', '\r']))
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "provide 1..256 nonempty tag names (1024 bytes each, 8192 aggregate), without NUL/newlines",
        ));
    }
    let mut tags = before.to_vec();
    match edit {
        TagEdit::Add(names) => {
            for name in names {
                if !tags.iter().any(|t| t.name == *name) {
                    tags.push(FileTag {
                        name: name.clone(),
                        color: Some(0),
                    });
                }
            }
        }
        TagEdit::Remove(names) => tags.retain(|t| !names.contains(&t.name)),
    }
    if tags.len() > 256 || tags.iter().map(|t| t.name.len()).sum::<usize>() > 16384 {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "result exceeds tag count/text budget",
        ));
    }
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_preserve_unselected_order_colors_and_exact_case() {
        let tags = vec![
            FileTag {
                name: "Keep".into(),
                color: Some(6),
            },
            FileTag {
                name: "Other".into(),
                color: None,
            },
        ];
        let added = edit_tags(
            &tags,
            &TagEdit::Add(vec!["Keep".into(), "keep".into(), "keep".into()]),
        )
        .unwrap();
        assert_eq!(&added[..2], tags);
        assert_eq!(added.len(), 3);
        assert_eq!(
            edit_tags(&added, &TagEdit::Remove(vec!["keep".into()])).unwrap(),
            tags
        );
    }
    #[test]
    fn malformed_and_oversized_names_are_rejected() {
        for names in [
            vec![],
            vec!["".into()],
            vec!["line\n6".into()],
            vec!["x".repeat(1025)],
            vec!["tag".into(); 257],
        ] {
            assert!(edit_tags(&[], &TagEdit::Add(names)).is_err());
        }
    }
}
