use super::*;
use crate::commands::{Cli, Command};
use clap::Parser;

fn parse(args: &[&str]) -> (PreviewRequest, u64) {
    let Command::Files {
        action: FilesAction::Preview { action },
    } = Cli::try_parse_from(std::iter::once("cueward").chain(args.iter().copied()))
        .unwrap()
        .command
    else {
        panic!("preview");
    };
    action.request()
}
#[test]
fn preview_formats_parse_scoped_budgets_and_pdf_range() {
    for (kind, expected) in [
        ("pdf", PreviewKind::Pdf),
        ("image", PreviewKind::Image),
        ("thumbnail", PreviewKind::Thumbnail),
    ] {
        let (request, timeout) = parse(&[
            "files",
            "preview",
            kind,
            "--root",
            "/tmp",
            "--path",
            "臺灣.pdf",
        ]);
        assert_eq!(request.options.kind, expected);
        assert_eq!(timeout, 10000);
        assert!(!request.options.ocr);
        assert!(request.options.render);
    }
    let (req, _) = parse(&[
        "files",
        "preview",
        "pdf",
        "--root",
        "/tmp",
        "--path",
        "a",
        "--start-page",
        "3",
        "--page-count",
        "2",
        "--ocr",
        "--no-render",
        "--expected-version",
        "v",
        "--follow-links",
    ]);
    assert_eq!((req.options.start_page, req.options.page_count), (3, 2));
    assert!(req.options.ocr && !req.options.render && req.follow_links);
    assert_eq!(req.expected_version.as_deref(), Some("v"));
}
#[test]
fn preview_requires_explicit_file_and_rejects_zero_or_unbounded_options() {
    assert!(
        Cli::try_parse_from(["cueward", "files", "preview", "image", "--root", "/tmp"]).is_err()
    );
    for (flag, value) in [
        ("--max-input-bytes", "0"),
        ("--max-input-bytes", "268435457"),
        ("--max-text-bytes", "1048577"),
        ("--max-dimension", "2049"),
        ("--max-preview-bytes", "33554433"),
        ("--start-page", "0"),
        ("--page-count", "21"),
        ("--timeout-ms", "30001"),
    ] {
        assert!(
            Cli::try_parse_from([
                "cueward", "files", "preview", "pdf", "--root", "/tmp", "--path", "a", flag, value
            ])
            .is_err()
        );
    }
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "preview",
            "thumbnail",
            "--root",
            "/tmp",
            "--path",
            "a",
            "--ocr"
        ])
        .is_err()
    );
}
