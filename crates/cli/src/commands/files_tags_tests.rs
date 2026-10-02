use crate::commands::Cli;
use clap::Parser;
#[test]
fn tag_read_add_remove_and_receipt_parse() {
    assert!(Cli::try_parse_from(["cueward", "files", "tags", "read", "--root", "/tmp"]).is_ok());
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "tags",
            "receipt",
            "--operation-id",
            "id"
        ])
        .is_ok()
    );
    for command in ["add", "remove"] {
        assert!(
            Cli::try_parse_from([
                "cueward",
                "files",
                "tags",
                command,
                "--root",
                "/tmp",
                "--path",
                "owned",
                "--expected-version",
                "v",
                "--expected-tags-version",
                "t",
                "--tag",
                "資料",
                "--tag",
                "<external>"
            ])
            .is_ok()
        );
    }
}
#[test]
fn tag_edits_require_versions_names_and_refuse_unsupported_flags() {
    for absent in [
        "--path",
        "--expected-version",
        "--expected-tags-version",
        "--tag",
    ] {
        let mut args = write_args("add");
        let index = args.iter().position(|a| *a == absent).unwrap();
        args.drain(index..index + 2);
        assert!(Cli::try_parse_from(args).is_err());
    }
    for extra in [
        vec!["--follow-links"],
        vec!["--color", "6"],
        vec!["--timeout-ms", "0"],
        vec!["--timeout-ms", "30001"],
    ] {
        let mut args = write_args("remove");
        args.extend(extra);
        assert!(Cli::try_parse_from(args).is_err());
    }
}

fn write_args(command: &str) -> Vec<&str> {
    vec![
        "cueward",
        "files",
        "tags",
        command,
        "--root",
        "/tmp",
        "--path",
        "owned",
        "--expected-version",
        "v",
        "--expected-tags-version",
        "t",
        "--tag",
        "name",
    ]
}
