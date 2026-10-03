use crate::commands::Cli;
use clap::Parser;
#[test]
fn rename_move_and_relocation_receipt_parse() {
    for (operation, flag) in [("rename", "--name"), ("move", "--destination")] {
        let args = [
            "cueward",
            "files",
            operation,
            "--root",
            "/tmp",
            "--path",
            "owned",
            flag,
            "新名稱",
            "--expected-version",
            "s",
            "--expected-parent-version",
            "p",
            "--dry-run",
        ];
        assert!(Cli::try_parse_from(args).is_ok());
        for bad in ["--overwrite", "--follow-links", "--recursive"] {
            assert!(Cli::try_parse_from(args.into_iter().chain([bad])).is_err());
        }
    }
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "relocation",
            "receipt",
            "--operation-id",
            "id"
        ])
        .is_ok()
    );
}
#[test]
fn relocation_requires_explicit_revisions_and_bounded_timeout() {
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "move",
            "--root",
            "/tmp",
            "--path",
            "a",
            "--destination",
            "b"
        ])
        .is_err()
    );
    for timeout in ["0", "30001"] {
        assert!(
            Cli::try_parse_from([
                "cueward",
                "files",
                "rename",
                "--root",
                "/tmp",
                "--path",
                "a",
                "--name",
                "b",
                "--expected-version",
                "s",
                "--expected-parent-version",
                "p",
                "--timeout-ms",
                timeout
            ])
            .is_err()
        );
    }
}

#[test]
fn relocation_link_itself_is_an_explicit_option_not_follow_links_or_a_receipt_option() {
    for (action, flag) in [("rename", "--name"), ("move", "--destination")] {
        let args = [
            "cueward",
            "files",
            action,
            "--root",
            "/tmp",
            "--path",
            "link",
            flag,
            "new",
            "--expected-version",
            "s",
            "--expected-parent-version",
            "p",
        ];
        assert!(Cli::try_parse_from(args).is_ok());
        assert!(Cli::try_parse_from(args.into_iter().chain(["--link-itself"])).is_ok());
        assert!(
            Cli::try_parse_from(args.into_iter().chain(["--link-itself", "--dry-run"])).is_ok()
        );
        assert!(
            Cli::try_parse_from(args.into_iter().chain(["--link-itself", "--follow-links"]))
                .is_err()
        );
    }
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "relocation",
            "receipt",
            "--operation-id",
            "id",
            "--link-itself"
        ])
        .is_err()
    );
}
