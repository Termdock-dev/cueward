use super::super::*;

pub(super) fn decode(
    bytes: &[u8],
    encoding: FileEncoding,
    eof: bool,
) -> Result<(String, usize), FileError> {
    match encoding {
        FileEncoding::Hex => Ok((
            bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
            bytes.len(),
        )),
        FileEncoding::Utf8 => utf8(bytes, eof),
        FileEncoding::Utf16Le | FileEncoding::Utf16Be => utf16(bytes, encoding, eof),
    }
}

fn utf8(bytes: &[u8], eof: bool) -> Result<(String, usize), FileError> {
    let used = match std::str::from_utf8(bytes) {
        Ok(_) => bytes.len(),
        Err(error) if error.error_len().is_none() && !eof => error.valid_up_to(),
        Err(_) => {
            return Err(FileError::new(
                FileErrorCode::DecodeError,
                "invalid UTF-8 at this range; select the encoding or hex explicitly",
            ));
        }
    };
    require_progress(bytes.len(), used)?;
    let content = std::str::from_utf8(&bytes[..used])
        .map_err(|e| FileError::new(FileErrorCode::DecodeError, e.to_string()))?;
    reject_nul(content)?;
    Ok((content.to_owned(), used))
}

fn utf16(bytes: &[u8], encoding: FileEncoding, eof: bool) -> Result<(String, usize), FileError> {
    if bytes.len() % 2 != 0 && eof {
        return Err(FileError::new(
            FileErrorCode::DecodeError,
            "incomplete UTF-16 code unit at EOF",
        ));
    }
    let mut units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| {
            if encoding == FileEncoding::Utf16Le {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        })
        .collect();
    if !eof
        && units
            .last()
            .is_some_and(|unit| (0xd800..=0xdbff).contains(unit))
    {
        units.pop();
    }
    let used = units.len() * 2;
    require_progress(bytes.len(), used)?;
    let content = String::from_utf16(&units).map_err(|_| {
        FileError::new(
            FileErrorCode::DecodeError,
            "invalid UTF-16 surrogate sequence at this range",
        )
    })?;
    reject_nul(&content)?;
    Ok((content, used))
}

fn reject_nul(content: &str) -> Result<(), FileError> {
    if content.contains('\0') {
        Err(FileError::new(
            FileErrorCode::BinaryData,
            "decoded text contains NUL; use hex for binary bytes",
        ))
    } else {
        Ok(())
    }
}

fn require_progress(length: usize, used: usize) -> Result<(), FileError> {
    if length > 0 && used == 0 {
        Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "max-bytes cannot fit the next complete character",
        ))
    } else {
        Ok(())
    }
}
