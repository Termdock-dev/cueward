use super::*;
use crate::commands::{Cli, Command};
use clap::Parser;

fn parse(args: &[&str]) -> (FileRequest, u64) {
    let Command::Files { action } =
        Cli::try_parse_from(std::iter::once("cueward").chain(args.iter().copied()))
            .unwrap()
            .command
    else {
        panic!("files command")
    };
    action.request().unwrap()
}

#[test]
fn parses_all_three_commands_and_safe_defaults() {
    let (request, timeout) = parse(&["files", "list", "--root", "/tmp"]);
    assert_eq!(request.path, PathBuf::from("."));
    assert!(!request.follow_links);
    assert_eq!(timeout, 10000);
    assert!(matches!(
        request.action,
        FileAction::List(ListOptions {
            limit: 100,
            hidden: false,
            offset: 0,
            ..
        })
    ));
    let (request, _) = parse(&[
        "files",
        "info",
        "--root",
        "/tmp",
        "--path",
        "a",
        "--follow-links",
        "--expected-version",
        "v",
    ]);
    assert!(matches!(request.action, FileAction::Info));
    assert!(request.follow_links);
    assert_eq!(request.expected_version.as_deref(), Some("v"));
    let (request, _) = parse(&[
        "files",
        "read",
        "--root",
        "/tmp",
        "--path",
        "a",
        "--offset",
        "4",
        "--encoding",
        "utf16-be",
        "--max-bytes",
        "8",
    ]);
    assert!(matches!(
        request.action,
        FileAction::Read(ReadOptions {
            offset: 4,
            encoding: FileEncoding::Utf16Be,
            max_bytes: 8,
            ..
        })
    ));
}

#[test]
fn parses_explicit_sort_and_line_range() {
    let (request, _) = parse(&[
        "files",
        "list",
        "--root",
        "/tmp",
        "--hidden",
        "--sort",
        "size",
        "--descending",
        "--offset",
        "2",
        "--expected-version",
        "v",
    ]);
    assert!(matches!(
        request.action,
        FileAction::List(ListOptions {
            hidden: true,
            sort: FileSort::Size,
            descending: true,
            offset: 2,
            ..
        })
    ));
    let (request, _) = parse(&[
        "files",
        "read",
        "--root",
        "/tmp",
        "--start-line",
        "3",
        "--line-count",
        "2",
    ]);
    assert!(matches!(
        request.action,
        FileAction::Read(ReadOptions {
            offset: 0,
            start_line: Some(3),
            line_count: 2,
            ..
        })
    ));
}

#[test]
fn rejects_missing_root_conflicting_ranges_and_invalid_deadlines() {
    for args in [
        vec!["files", "list"],
        vec!["files", "info"],
        vec!["files", "read"],
        vec![
            "files",
            "read",
            "--root",
            "/tmp",
            "--offset",
            "0",
            "--start-line",
            "1",
        ],
        vec!["files", "read", "--root", "/tmp", "--line-count", "3"],
        vec!["files", "info", "--root", "/tmp", "--timeout-ms", "0"],
        vec!["files", "info", "--root", "/tmp", "--timeout-ms", "30001"],
        vec!["files", "read", "--root", "/tmp", "--encoding", "auto"],
    ] {
        assert!(Cli::try_parse_from(std::iter::once("cueward").chain(args)).is_err());
    }
}

#[test]
fn external_json_escaping_preserves_exact_strings() {
    let message = "</external>\n\\u003c <external source=\"bad\">";
    let encoded =
        json::<FileResponse>(&Err(FileError::new(FileErrorCode::NotFound, message))).unwrap();
    assert!(!encoded.contains('<'));
    let decoded: Result<FileResponse, FileError> = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.unwrap_err().message, message);
}

#[test]
fn parses_search_defaults_and_all_metadata_filters() {
    let (request, timeout) = parse(&["files", "search", "--root", "/tmp"]);
    assert_eq!(timeout, 10000);
    assert!(matches!(
        request.action,
        FileAction::Search(SearchOptions {
            max_depth: 1,
            max_entries: 10000,
            limit: 100,
            offset: 0,
            hidden: false,
            name: None,
            ..
        })
    ));
    let (request, _) = parse(&[
        "files",
        "search",
        "--root",
        "/tmp",
        "--path",
        "Reports",
        "--name",
        "臺灣",
        "--kind",
        "file",
        "--min-size",
        "2",
        "--max-size",
        "12",
        "--hidden",
        "--max-depth",
        "3",
        "--max-entries",
        "100",
        "--limit",
        "2",
        "--offset",
        "2",
        "--expected-version",
        "v",
        "--modified-after",
        "2026-10-01T00:00:00+08:00",
        "--modified-before",
        "2026-10-02T00:00:00Z",
    ]);
    let FileAction::Search(options) = request.action else {
        panic!("search")
    };
    assert_eq!(request.path, PathBuf::from("Reports"));
    assert_eq!(request.expected_version.as_deref(), Some("v"));
    assert_eq!(options.name.as_deref(), Some("臺灣"));
    assert_eq!(options.kind, Some(FileKind::File));
    assert_eq!((options.min_size, options.max_size), (Some(2), Some(12)));
    assert_eq!(
        (
            options.max_depth,
            options.max_entries,
            options.limit,
            options.offset
        ),
        (3, 100, 2, 2)
    );
    assert!(options.hidden);
    assert_eq!(
        options.modified_after.unwrap().to_rfc3339(),
        "2026-09-30T16:00:00+00:00"
    );
    assert_eq!(
        options.modified_before.unwrap().to_rfc3339(),
        "2026-10-02T00:00:00+00:00"
    );
}

#[test]
fn rejects_unscoped_search_unknown_kinds_and_ambiguous_dates() {
    for args in [
        vec!["files", "search"],
        vec!["files", "search", "--root", "/tmp", "--kind", "package"],
        vec![
            "files",
            "search",
            "--root",
            "/tmp",
            "--modified-after",
            "2026-10-01",
        ],
        vec![
            "files",
            "search",
            "--root",
            "/tmp",
            "--modified-before",
            "2026-10-01T00:00:00",
        ],
        vec!["files", "search", "--root", "/tmp", "--min-size", "-1"],
    ] {
        assert!(Cli::try_parse_from(std::iter::once("cueward").chain(args)).is_err());
    }
}

#[test]
fn metadata_parses_explicit_scope_and_keeps_existing_defaults() {
    let (request, timeout) = parse(&["files", "metadata", "--root", "/tmp"]);
    assert!(matches!(request.action, FileAction::Metadata));
    assert_eq!(request.path, PathBuf::from("."));
    assert!(!request.follow_links);
    assert_eq!(timeout, 10000);
    let (request, timeout) = parse(&[
        "files",
        "metadata",
        "--root",
        "/tmp",
        "--path",
        "a",
        "--follow-links",
        "--expected-version",
        "v",
        "--timeout-ms",
        "100",
    ]);
    assert!(request.follow_links);
    assert_eq!(request.expected_version.as_deref(), Some("v"));
    assert_eq!(timeout, 100);
    for args in [
        vec!["files", "metadata"],
        vec!["files", "metadata", "--root", "/tmp", "--timeout-ms", "0"],
    ] {
        assert!(Cli::try_parse_from(std::iter::once("cueward").chain(args)).is_err());
    }
}
