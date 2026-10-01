use super::*;
use crate::commands::{Cli, Command};
use clap::Parser;

fn parse(args: &[&str]) -> (FinderRequest, u64) {
    let Command::Files {
        action: FilesAction::Finder { action },
    } = Cli::try_parse_from(std::iter::once("cueward").chain(args.iter().copied()))
        .unwrap()
        .command
    else {
        panic!("Finder command")
    };
    action.request()
}

#[test]
fn finder_context_and_reveal_parse_explicit_scopes_and_budgets() {
    let (request, timeout) = parse(&["files", "finder", "context", "--root", "/tmp"]);
    assert!(matches!(
        request.action,
        FinderAction::Context { max_items: 100 }
    ));
    assert_eq!(timeout, 10000);
    let (request, timeout) = parse(&[
        "files",
        "finder",
        "context",
        "--root",
        "/tmp",
        "--max-items",
        "500",
        "--timeout-ms",
        "100",
    ]);
    assert!(matches!(
        request.action,
        FinderAction::Context { max_items: 500 }
    ));
    assert_eq!(timeout, 100);
    let (request, _) = parse(&[
        "files",
        "finder",
        "reveal",
        "--root",
        "/tmp",
        "--path",
        "臺灣.txt",
        "--expected-version",
        "v",
        "--follow-links",
    ]);
    assert!(
        matches!(request.action, FinderAction::Reveal { path, follow_links: true, expected_version: Some(v) } if path == std::path::Path::new("臺灣.txt") && v == "v")
    );
}

#[test]
fn finder_rejects_missing_scope_path_and_out_of_range_budgets() {
    for args in [
        vec!["files", "finder", "context"],
        vec!["files", "finder", "reveal", "--root", "/tmp"],
        vec!["files", "finder", "reveal", "--path", "a"],
        vec![
            "files",
            "finder",
            "context",
            "--root",
            "/tmp",
            "--max-items",
            "0",
        ],
        vec![
            "files",
            "finder",
            "context",
            "--root",
            "/tmp",
            "--max-items",
            "501",
        ],
        vec![
            "files",
            "finder",
            "context",
            "--root",
            "/tmp",
            "--timeout-ms",
            "0",
        ],
        vec![
            "files",
            "finder",
            "reveal",
            "--root",
            "/tmp",
            "--path",
            "a",
            "--timeout-ms",
            "30001",
        ],
    ] {
        assert!(Cli::try_parse_from(std::iter::once("cueward").chain(args)).is_err());
    }
}
