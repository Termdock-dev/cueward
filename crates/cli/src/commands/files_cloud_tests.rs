use super::*;
use crate::commands::{Cli, Command};
use clap::Parser;

fn parse(args: &[&str]) -> (CloudRequest, u64) {
    let Command::Files {
        action: FilesAction::Cloud { action },
    } = Cli::try_parse_from(std::iter::once("cueward").chain(args.iter().copied()))
        .unwrap()
        .command
    else {
        panic!("cloud");
    };
    action.request()
}

#[test]
fn cloud_status_and_download_parse_explicit_scope_and_revision() {
    let (req, timeout) = parse(&["files", "cloud", "status", "--root", "/tmp"]);
    assert_eq!(req.path, std::path::Path::new("."));
    assert!(matches!(req.action, CloudAction::Status));
    assert_eq!(timeout, 10000);
    let (req, timeout) = parse(&[
        "files",
        "cloud",
        "download",
        "--root",
        "/tmp",
        "--path",
        "臺灣.txt",
        "--expected-version",
        "v",
        "--max-bytes",
        "1024",
        "--follow-links",
        "--timeout-ms",
        "200",
    ]);
    assert_eq!(req.expected_version.as_deref(), Some("v"));
    assert!(matches!(
        req.action,
        CloudAction::Download { max_bytes: 1024 }
    ));
    assert!(req.follow_links);
    assert_eq!(timeout, 200);
}

#[test]
fn cloud_download_requires_file_and_version_and_rejects_invalid_budgets() {
    for args in [
        vec!["files", "cloud", "status"],
        vec!["files", "cloud", "download", "--root", "/tmp"],
        vec![
            "files", "cloud", "download", "--root", "/tmp", "--path", "a",
        ],
    ] {
        assert!(Cli::try_parse_from(std::iter::once("cueward").chain(args)).is_err());
    }
    for (flag, value) in [
        ("--max-bytes", "0"),
        ("--max-bytes", "268435457"),
        ("--timeout-ms", "0"),
        ("--timeout-ms", "30001"),
    ] {
        assert!(
            Cli::try_parse_from([
                "cueward",
                "files",
                "cloud",
                "download",
                "--root",
                "/tmp",
                "--path",
                "a",
                "--expected-version",
                "v",
                flag,
                value
            ])
            .is_err()
        );
    }
}
