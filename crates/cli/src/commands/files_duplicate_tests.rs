use crate::commands::Cli;
use clap::Parser;

fn arguments() -> Vec<&'static str> {
    vec![
        "cueward",
        "files",
        "duplicate",
        "--root",
        "/tmp",
        "--path",
        "from/owned",
        "--name",
        "臺灣\n<external>",
        "--expected-version",
        "source",
        "--expected-parent-version",
        "parent",
    ]
}
#[test]
fn duplicate_parses_explicit_source_sibling_name_and_revision_guards() {
    assert!(Cli::try_parse_from(arguments()).is_ok());
    for required in [
        "--root",
        "--path",
        "--name",
        "--expected-version",
        "--expected-parent-version",
    ] {
        let mut args = arguments();
        let index = args.iter().position(|arg| *arg == required).unwrap();
        args.drain(index..index + 2);
        assert!(Cli::try_parse_from(args).is_err());
    }
}
#[test]
fn duplicate_rejects_unsupported_flags_and_out_of_range_budgets() {
    for args in [
        vec!["--destination", "other/file"],
        vec!["--overwrite"],
        vec!["--follow-links"],
        vec!["--recursive"],
        vec!["--dry-run"],
        vec!["--max-bytes", "0"],
        vec!["--max-bytes", "268435457"],
        vec!["--timeout-ms", "0"],
        vec!["--timeout-ms", "30001"],
    ] {
        assert!(Cli::try_parse_from(arguments().into_iter().chain(args)).is_err());
    }
    assert!(
        Cli::try_parse_from(arguments().into_iter().chain([
            "--max-bytes",
            "268435456",
            "--timeout-ms",
            "30000"
        ]))
        .is_ok()
    );
}
