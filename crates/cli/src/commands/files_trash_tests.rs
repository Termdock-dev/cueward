use crate::commands::Cli;
use clap::Parser;
fn args() -> Vec<&'static str> {
    vec![
        "cueward",
        "files",
        "trash",
        "plan",
        "--root",
        "/owned",
        "--path",
        "臺灣\n<external>",
        "--expected-version",
        "observed",
    ]
}
#[test]
fn trash_plan_parsing_requires_an_explicit_selection_and_accepts_whole_worker_bounds() {
    assert!(Cli::try_parse_from(args()).is_ok());
    for value in ["1", "30000"] {
        assert!(Cli::try_parse_from(args().into_iter().chain(["--timeout-ms", value])).is_ok());
    }
    for option in ["--root", "--path", "--expected-version"] {
        let mut input = args();
        let index = input.iter().position(|a| *a == option).unwrap();
        input.drain(index..index + 2);
        assert!(Cli::try_parse_from(input).is_err());
    }
    for value in ["0", "30001"] {
        assert!(Cli::try_parse_from(args().into_iter().chain(["--timeout-ms", value])).is_err());
    }
}
#[test]
fn trash_plan_parsing_never_accepts_confirmation_execution_restore_or_link_following() {
    for flags in [
        vec!["--confirm"],
        vec!["--force"],
        vec!["--recursive"],
        vec!["--follow-links"],
        vec!["--destination", "/Trash"],
    ] {
        assert!(Cli::try_parse_from(args().into_iter().chain(flags)).is_err());
    }
    for command in ["execute", "receipt", "restore", "delete"] {
        let mut input = args();
        input[3] = command;
        assert!(Cli::try_parse_from(input).is_err());
    }
}
#[test]
fn trash_execute_requires_confirmation_parent_guard_and_accepts_bounded_backup() {
    let mut input = args();
    input[3] = "execute";
    assert!(Cli::try_parse_from(input.clone()).is_err());
    input.extend(["--expected-parent-version", "parent", "--confirm"]);
    assert!(Cli::try_parse_from(input.clone()).is_ok());
    for value in ["1", "268435456"] {
        assert!(
            Cli::try_parse_from(input.clone().into_iter().chain(["--max-bytes", value])).is_ok()
        );
    }
    for value in ["0", "268435457"] {
        assert!(
            Cli::try_parse_from(input.clone().into_iter().chain(["--max-bytes", value])).is_err()
        );
    }
    for flag in [
        "--force",
        "--recursive",
        "--follow-links",
        "--delete",
        "--destination",
    ] {
        assert!(Cli::try_parse_from(input.clone().into_iter().chain([flag])).is_err());
    }
}
#[test]
fn trash_receipt_requires_an_id_and_exposes_no_resume() {
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "trash",
            "receipt",
            "--operation-id",
            "id"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["cueward", "files", "trash", "receipt"]).is_err());
    for command in ["restore", "delete", "resume", "retry"] {
        assert!(
            Cli::try_parse_from(["cueward", "files", "trash", command, "--operation-id", "id"])
                .is_err()
        );
    }
}

fn restore_args() -> Vec<&'static str> {
    vec![
        "cueward",
        "files",
        "trash",
        "restore",
        "--operation-id",
        "id",
        "--root",
        "/owned",
        "--expected-parent-version",
        "parent",
    ]
}
#[test]
fn trash_restore_requires_original_id_root_and_fresh_parent_and_has_no_overwrite_or_delete_flags() {
    assert!(Cli::try_parse_from(restore_args()).is_ok());
    for option in ["--operation-id", "--root", "--expected-parent-version"] {
        let mut args = restore_args();
        let index = args.iter().position(|a| *a == option).unwrap();
        args.drain(index..index + 2);
        assert!(Cli::try_parse_from(args).is_err());
    }
    for flag in [
        "--confirm",
        "--force",
        "--recursive",
        "--follow-links",
        "--path",
        "--destination",
        "--max-bytes",
    ] {
        assert!(Cli::try_parse_from(restore_args().into_iter().chain([flag])).is_err());
    }
    for value in ["1", "30000"] {
        assert!(
            Cli::try_parse_from(restore_args().into_iter().chain(["--timeout-ms", value])).is_ok()
        );
    }
    for value in ["0", "30001"] {
        assert!(
            Cli::try_parse_from(restore_args().into_iter().chain(["--timeout-ms", value])).is_err()
        );
    }
}
#[test]
fn trash_restore_receipt_is_read_only_and_requires_its_own_id() {
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "trash",
            "restore-receipt",
            "--operation-id",
            "id"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["cueward", "files", "trash", "restore-receipt"]).is_err());
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "trash",
            "restore-receipt",
            "--operation-id",
            "id",
            "--retry"
        ])
        .is_err()
    );
}
