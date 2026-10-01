use super::scope::{Resolved, Scope};
use super::*;
use std::io::{BufReader, Read, Seek, SeekFrom};

mod decode;

pub(super) fn read(
    scope: &Scope<'_, impl FilePlatform>,
    resolved: &Resolved,
    info: FileInfo,
    options: &ReadOptions,
) -> Result<FileRead, FileError> {
    validate(&info, options)?;
    let mut file = scope.platform.open_regular(&resolved.path)?;
    verify(scope, &file, &info)?;
    let (offset, bytes) = if let Some(line) = options.start_line {
        lines(&mut file, line, options)?
    } else {
        file.seek(SeekFrom::Start(options.offset))?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(options.max_bytes as u64)
            .read_to_end(&mut bytes)?;
        (options.offset, bytes)
    };
    verify(scope, &file, &info)?;
    result(info, options, offset, bytes)
}

fn result(
    info: FileInfo,
    options: &ReadOptions,
    offset: u64,
    bytes: Vec<u8>,
) -> Result<FileRead, FileError> {
    let at_eof = offset + bytes.len() as u64 == info.size;
    let (content, consumed) = decode::decode(&bytes, options.encoding, at_eof)?;
    let end = offset + consumed as u64;
    let eof = end == info.size;
    let completed = bytes[..consumed].iter().filter(|&&b| b == b'\n').count()
        + usize::from(eof && consumed > 0 && bytes[consumed - 1] != b'\n');
    Ok(FileRead {
        file: info,
        encoding: options.encoding,
        content,
        offset,
        bytes_read: consumed,
        next_offset: (!eof).then_some(end),
        eof,
        truncated: !eof,
        start_line: options.start_line,
        lines_complete: options.start_line.map(|_| completed),
        next_line: options
            .start_line
            .and_then(|line| (!eof).then_some(line + completed as u64)),
    })
}

fn validate(info: &FileInfo, options: &ReadOptions) -> Result<(), FileError> {
    if info.kind != FileKind::File {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "read requires a regular file",
        ));
    }
    if info.data_state == DataState::Dataless {
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            "file is dataless; no download is requested",
        ));
    }
    if options.max_bytes == 0 || options.max_bytes > MAX_READ_BYTES || options.offset > info.size {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "max-bytes must be 1..1048576 and offset must not exceed file size",
        ));
    }
    if options.start_line.is_some_and(|line| line == 0)
        || (options.start_line.is_some()
            && (options.offset != 0
                || options.encoding != FileEncoding::Utf8
                || options.line_count == 0
                || options.line_count > 10000))
        || (matches!(
            options.encoding,
            FileEncoding::Utf16Le | FileEncoding::Utf16Be
        ) && options.offset % 2 != 0)
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "line reads require UTF-8, start-line >= 1, line-count 1..10000 and no byte offset; UTF-16 offset must be even",
        ));
    }
    Ok(())
}

fn verify(
    scope: &Scope<'_, impl FilePlatform>,
    file: &std::fs::File,
    info: &FileInfo,
) -> Result<(), FileError> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || scope.platform.stamp(&metadata).version != info.version {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "opened file identity or metadata changed; content discarded",
        ));
    }
    Ok(())
}

fn byte(reader: &mut impl Read) -> Result<Option<u8>, FileError> {
    let mut value = [0];
    Ok((reader.read(&mut value)? != 0).then_some(value[0]))
}

fn lines(
    file: &mut std::fs::File,
    start: u64,
    options: &ReadOptions,
) -> Result<(u64, Vec<u8>), FileError> {
    let mut reader = BufReader::new(file);
    let mut line = 1;
    let mut offset = 0;
    while line < start {
        if offset == MAX_SCAN_BYTES {
            return Err(FileError::new(
                FileErrorCode::ScanLimit,
                "line prefix exceeds 8 MiB; use a byte offset",
            ));
        }
        let value = byte(&mut reader)?.ok_or_else(|| {
            FileError::new(
                FileErrorCode::InvalidOptions,
                "start-line exceeds file lines",
            )
        })?;
        offset += 1;
        if value == b'\n' {
            line += 1;
        }
    }
    let mut bytes = Vec::new();
    let mut completed = 0;
    while bytes.len() < options.max_bytes && completed < options.line_count {
        let Some(value) = byte(&mut reader)? else {
            break;
        };
        bytes.push(value);
        if value == b'\n' {
            completed += 1;
        }
    }
    if bytes.is_empty() && start > 1 {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "start-line exceeds file lines",
        ));
    }
    Ok((offset, bytes))
}
