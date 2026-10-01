use super::*;
use crate::commands::{Cli, Command};
use clap::Parser;

fn parse(args: &[&str]) -> (SpotlightRequest, u64) {
    let Command::Files {
        action: FilesAction::Spotlight(args),
    } = Cli::try_parse_from(std::iter::once("cueward").chain(args.iter().copied()))
        .unwrap()
        .command
    else {
        panic!("Spotlight command");
    };
    args.request()
}

#[test]
fn spotlight_requires_explicit_scope_and_text_with_bounded_defaults() {
    let (req, timeout) = parse(&[
        "files",
        "spotlight",
        "--root",
        "/tmp",
        "--text",
        "臺灣 report",
    ]);
    assert_eq!(req.root, std::path::Path::new("/tmp"));
    assert_eq!(req.text, "臺灣 report");
    assert_eq!(req.max_depth, 1);
    assert!(!req.hidden);
    assert_eq!(req.max_candidates, 10000);
    assert_eq!(req.limit, 100);
    assert_eq!(timeout, 10000);
    let (req, timeout) = parse(&[
        "files",
        "spotlight",
        "--root",
        "/tmp",
        "--text",
        "report",
        "--hidden",
        "--max-depth",
        "32",
        "--max-candidates",
        "500",
        "--limit",
        "20",
        "--timeout-ms",
        "200",
    ]);
    assert!(req.hidden);
    assert_eq!(req.max_depth, 32);
    assert_eq!(req.max_candidates, 500);
    assert_eq!(req.limit, 20);
    assert_eq!(timeout, 200);
}

#[test]
fn spotlight_rejects_missing_scope_text_invalid_bounds_and_filesystem_resume_flags() {
    for args in [
        vec!["files", "spotlight"],
        vec!["files", "spotlight", "--root", "/tmp"],
        vec!["files", "spotlight", "--text", "x"],
    ] {
        assert!(Cli::try_parse_from(std::iter::once("cueward").chain(args)).is_err());
    }
    for (flag, value) in [
        ("--max-depth", "0"),
        ("--max-depth", "33"),
        ("--max-candidates", "0"),
        ("--max-candidates", "10001"),
        ("--limit", "0"),
        ("--limit", "501"),
        ("--timeout-ms", "0"),
        ("--timeout-ms", "30001"),
        ("--offset", "1"),
        ("--expected-version", "v"),
    ] {
        assert!(
            Cli::try_parse_from([
                "cueward",
                "files",
                "spotlight",
                "--root",
                "/tmp",
                "--text",
                "x",
                flag,
                value
            ])
            .is_err()
        );
    }
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "spotlight",
            "--root",
            "/tmp",
            "--text",
            "x",
            "--follow-links"
        ])
        .is_err()
    );
}
