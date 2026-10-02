//! Only the observed bounded binary string-array tag representation is editable.
use super::*;
use sha2::{Digest, Sha256};

pub(super) struct Stored {
    pub raw: Option<Vec<u8>>,
    pub tags: Vec<FileTag>,
    pub version: String,
}
impl Stored {
    /// Preserve absence separately from an empty stored array.
    pub(super) fn decode(raw: Option<Vec<u8>>) -> Result<Self, FileError> {
        let mut hash = Sha256::new();
        hash.update([u8::from(raw.is_some())]);
        let tags = match &raw {
            None => vec![],
            Some(bytes) => {
                hash.update(bytes);
                decode(bytes)?
            }
        };
        Ok(Self {
            raw,
            tags,
            version: format!("{:x}", hash.finalize()),
        })
    }
}
fn decode(bytes: &[u8]) -> Result<Vec<FileTag>, FileError> {
    if bytes.len() < 40 || bytes.len() > 65536 || !bytes.starts_with(b"bplist00") {
        return Err(unsupported("tags are not a supported bounded binary plist"));
    }
    let count = u64::from_be_bytes(
        bytes[bytes.len() - 24..bytes.len() - 16]
            .try_into()
            .map_err(|_| unsupported("invalid plist trailer"))?,
    );
    if count > 1024 {
        return Err(unsupported("tag plist object count exceeds budget"));
    }
    let value = plist::Value::from_reader(std::io::Cursor::new(bytes))
        .map_err(|_| unsupported("invalid tag plist"))?;
    let array = value
        .as_array()
        .ok_or_else(|| unsupported("tags are not a string array"))?;
    if array.len() > 256 {
        return Err(unsupported("too many stored tags"));
    }
    let tags = array
        .iter()
        .map(|v| parse(v.as_string().ok_or_else(|| unsupported("non-string tag"))?))
        .collect::<Result<Vec<_>, _>>()?;
    if tags.iter().map(|t| t.name.len()).sum::<usize>() > 16384 {
        return Err(unsupported("stored tag text exceeds budget"));
    }
    Ok(tags)
}
fn parse(value: &str) -> Result<FileTag, FileError> {
    let (name, color) = match value.rsplit_once('\n') {
        Some((name, suffix))
            if suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'0'..=b'7') =>
        {
            (name, Some(suffix.as_bytes()[0] - b'0'))
        }
        Some(_) => return Err(unsupported("unknown tag color representation")),
        None => (value, None),
    };
    if name.is_empty() || name.len() > 1024 || name.contains(['\0', '\n', '\r']) {
        return Err(unsupported("unsupported stored tag name"));
    }
    Ok(FileTag {
        name: name.to_owned(),
        color,
    })
}
/// Preserve each existing string's name/color representation; new tags are uncolored.
pub(super) fn encode(tags: &[FileTag]) -> Result<Vec<u8>, FileError> {
    let array = tags
        .iter()
        .map(|t| {
            plist::Value::String(match t.color {
                Some(color) => format!("{}\n{color}", t.name),
                None => t.name.clone(),
            })
        })
        .collect();
    let mut bytes = Vec::new();
    plist::Value::Array(array)
        .to_writer_binary(&mut bytes)
        .map_err(|_| unsupported("cannot encode tags"))?;
    if bytes.len() > 65536 {
        return Err(unsupported("encoded tags exceed budget"));
    }
    Ok(bytes)
}
fn unsupported(message: &str) -> FileError {
    FileError::new(FileErrorCode::UnsupportedType, message)
}
