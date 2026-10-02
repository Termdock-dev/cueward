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
