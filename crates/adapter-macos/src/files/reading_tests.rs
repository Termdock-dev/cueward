use super::tests::Fixture;
use super::*;
use std::fs;
use std::os::unix::fs::symlink;

pub(super) fn options() -> ReadOptions {
    ReadOptions {
        offset: 0,
        max_bytes: 65536,
        encoding: FileEncoding::Utf8,
        start_line: None,
        line_count: 100,
    }
}

fn read(f: &Fixture, opts: ReadOptions) -> Result<FileRead, FileError> {
    let FileResponse::Read(value) = f.run("a", FileAction::Read(opts))? else {
        panic!("read")
    };
    Ok(value)
}

#[test]
fn utf8_byte_ranges_resume_without_splitting_characters() {
    let f = Fixture::new();
    f.write("a", "A臺灣B");
    let mut opts = options();
    opts.max_bytes = 5;
    let first = read(&f, opts.clone()).unwrap();
    assert_eq!(first.content, "A臺");
    assert_eq!(first.bytes_read, 4);
    assert_eq!(first.next_offset, Some(4));
    assert!(first.truncated);
    opts.offset = 4;
    let second = read(&f, opts.clone()).unwrap();
    assert_eq!(second.content, "灣B");
    assert!(second.eof);
    assert!(!second.truncated);
    opts.offset = 8;
    let end = read(&f, opts.clone()).unwrap();
    assert_eq!(end.content, "");
    assert_eq!(end.bytes_read, 0);
    assert!(end.eof);
    opts.offset = 2;
    assert_eq!(read(&f, opts).unwrap_err().code, FileErrorCode::DecodeError);
    let mut opts = options();
    opts.offset = 1;
    opts.max_bytes = 2;
    assert_eq!(
        read(&f, opts).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}

#[test]
fn strict_decoding_reports_binary_invalid_and_incomplete_text() {
    let f = Fixture::new();
    for (bytes, code) in [
        (vec![0, 65], FileErrorCode::BinaryData),
        (vec![0xff], FileErrorCode::DecodeError),
        (vec![0xe8, 0x87], FileErrorCode::DecodeError),
    ] {
        f.write("a", bytes);
        assert_eq!(read(&f, options()).unwrap_err().code, code);
    }
    let mut opts = options();
    opts.encoding = FileEncoding::Hex;
    assert_eq!(read(&f, opts).unwrap().content, "e887");
    f.write("a", "");
    assert!(read(&f, options()).unwrap().eof);
}

#[test]
fn utf16_both_endiannesses_resume_surrogate_pairs_and_reject_invalid_ranges() {
    let f = Fixture::new();
    for encoding in [FileEncoding::Utf16Le, FileEncoding::Utf16Be] {
        let bytes: Vec<u8> = "A𠀀B"
            .encode_utf16()
            .flat_map(|unit| {
                if encoding == FileEncoding::Utf16Le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                }
            })
            .collect();
        f.write("a", bytes);
        let mut opts = options();
        opts.encoding = encoding;
        opts.max_bytes = 5;
        let first = read(&f, opts.clone()).unwrap();
        assert_eq!(first.content, "A");
        assert_eq!(first.next_offset, Some(2));
        opts.offset = 2;
        opts.max_bytes = 6;
        assert_eq!(read(&f, opts.clone()).unwrap().content, "𠀀B");
        opts.offset = 1;
        assert_eq!(
            read(&f, opts).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
    }
    for bytes in [vec![65], vec![0, 0xd8], vec![0, 0xdc]] {
        f.write("a", bytes);
        let mut opts = options();
        opts.encoding = FileEncoding::Utf16Le;
        assert_eq!(read(&f, opts).unwrap_err().code, FileErrorCode::DecodeError);
    }
}

#[test]
fn line_reads_preserve_crlf_and_report_partial_line_byte_resume() {
    let f = Fixture::new();
    f.write("a", "one\r\n臺灣\nlast");
    let mut opts = options();
    opts.start_line = Some(2);
    opts.line_count = 1;
    let second = read(&f, opts.clone()).unwrap();
    assert_eq!(second.content, "臺灣\n");
    assert_eq!(second.offset, 5);
    assert_eq!(second.lines_complete, Some(1));
    assert_eq!(second.next_line, Some(3));
    assert_eq!(second.next_offset, Some(12));
    opts.max_bytes = 4;
    let partial = read(&f, opts.clone()).unwrap();
    assert_eq!(partial.content, "臺");
    assert_eq!(partial.next_offset, Some(8));
    assert_eq!(partial.lines_complete, Some(0));
    assert_eq!(partial.next_line, Some(2));
    opts.start_line = Some(3);
    opts.max_bytes = 100;
    let last = read(&f, opts.clone()).unwrap();
    assert_eq!(last.content, "last");
    assert!(last.eof);
    assert_eq!(last.lines_complete, Some(1));
    opts.start_line = Some(4);
    assert_eq!(
        read(&f, opts).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
    f.write("a", "");
    let mut opts = options();
    opts.start_line = Some(1);
    assert!(read(&f, opts).unwrap().eof);
}

#[test]
fn line_prefix_is_bounded_and_invalid_limits_are_rejected() {
    let f = Fixture::new();
    f.write("a", vec![b'a'; MAX_SCAN_BYTES as usize + 1]);
    let mut opts = options();
    opts.start_line = Some(2);
    assert_eq!(read(&f, opts).unwrap_err().code, FileErrorCode::ScanLimit);
    for opts in [
        ReadOptions {
            max_bytes: 0,
            ..options()
        },
        ReadOptions {
            max_bytes: MAX_READ_BYTES + 1,
            ..options()
        },
        ReadOptions {
            offset: MAX_SCAN_BYTES + 2,
            ..options()
        },
        ReadOptions {
            start_line: Some(0),
            ..options()
        },
        ReadOptions {
            start_line: Some(1),
            offset: 1,
            ..options()
        },
        ReadOptions {
            start_line: Some(1),
            encoding: FileEncoding::Hex,
            ..options()
        },
        ReadOptions {
            start_line: Some(1),
            line_count: 0,
            ..options()
        },
        ReadOptions {
            start_line: Some(1),
            line_count: 10001,
            ..options()
        },
    ] {
        assert_eq!(
            read(&f, opts).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
    }
}

#[test]
fn refuses_special_files_and_links_before_reading() {
    let f = Fixture::new();
    fs::create_dir(f.0.path().join("a")).unwrap();
    assert_eq!(
        read(&f, options()).unwrap_err().code,
        FileErrorCode::UnsupportedType
    );
    fs::remove_dir(f.0.path().join("a")).unwrap();
    symlink("missing", f.0.path().join("a")).unwrap();
    assert_eq!(
        read(&f, options()).unwrap_err().code,
        FileErrorCode::SymlinkDisallowed
    );
    fs::remove_file(f.0.path().join("a")).unwrap();
    let status = std::process::Command::new("/usr/bin/mkfifo")
        .arg(f.0.path().join("a"))
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        read(&f, options()).unwrap_err().code,
        FileErrorCode::UnsupportedType
    );
}

#[test]
fn changed_open_identity_discards_data_and_stale_version_is_rejected() {
    struct Replace;
    impl FilePlatform for Replace {
        fn stamp(&self, metadata: &Metadata) -> FileStamp {
            MacFiles.stamp(metadata)
        }
        fn open_regular(&self, path: &Path) -> io::Result<File> {
            fs::rename(path, path.with_extension("old"))?;
            fs::write(path, b"replacement secret")?;
            MacFiles.open_regular(path)
        }
    }
    let f = Fixture::new();
    f.write("a", "original");
    let request = f.request("a", FileAction::Read(options()));
    assert_eq!(
        cueward_core::files::execute(&Replace, &request)
            .unwrap_err()
            .code,
        FileErrorCode::Changed
    );
    let FileResponse::Info(info) = f.run("a", FileAction::Info).unwrap() else {
        panic!("info")
    };
    let mut request = request;
    request.expected_version = Some(info.version);
    f.write("a", "modified");
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::Changed
    );
}
