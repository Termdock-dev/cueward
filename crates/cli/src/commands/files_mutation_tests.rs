use crate::commands::Cli;
use clap::Parser;

#[test]
fn mkdir_copy_and_receipt_parse_with_explicit_revision_guards() {
    for args in [
        vec![
            "cueward",
            "files",
            "mkdir",
            "--root",
            "/tmp",
            "--path",
            "新資料夾",
            "--expected-parent-version",
            "parent",
        ],
        vec![
            "cueward",
            "files",
            "copy",
            "--root",
            "/tmp",
            "--path",
            "來源",
            "--destination",
            "目的",
            "--expected-version",
            "source",
            "--expected-parent-version",
            "parent",
        ],
        vec!["cueward", "files", "receipt", "--operation-id", "id"],
    ] {
        assert!(Cli::try_parse_from(args).is_ok());
    }
}
#[test]
fn mutation_parsing_requires_scope_destinations_revisions_and_bounded_limits() {
    for args in [
        vec!["cueward", "files", "mkdir", "--root", "/tmp", "--path", "a"],
        vec![
            "cueward",
            "files",
            "copy",
            "--root",
            "/tmp",
            "--path",
            "a",
            "--destination",
            "b",
            "--expected-parent-version",
            "p",
        ],
        vec!["cueward", "files", "receipt"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
    for (flag, value) in [
        ("--max-bytes", "0"),
        ("--max-bytes", "268435457"),
        ("--timeout-ms", "0"),
        ("--timeout-ms", "30001"),
        ("--follow-links", "true"),
        ("--overwrite", "true"),
    ] {
        assert!(
            Cli::try_parse_from([
                "cueward",
                "files",
                "copy",
                "--root",
                "/tmp",
                "--path",
                "a",
                "--destination",
                "b",
                "--expected-version",
                "v",
                "--expected-parent-version",
                "p",
                flag,
                value
            ])
            .is_err()
        );
    }
}
