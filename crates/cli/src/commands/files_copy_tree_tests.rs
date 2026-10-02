use crate::commands::Cli;
use clap::Parser;
fn args() -> Vec<&'static str> {
    vec![
        "cueward",
        "files",
        "copy-tree",
        "plan",
        "--root",
        "/tmp",
        "--path",
        "source",
        "--destination",
        "copy",
        "--expected-version",
        "source",
        "--expected-parent-version",
        "parent",
    ]
}
#[test]
fn copy_tree_plan_requires_scope_paths_and_both_revision_guards() {
    assert!(Cli::try_parse_from(args()).is_ok());
    for required in [
        "--root",
        "--path",
        "--destination",
        "--expected-version",
        "--expected-parent-version",
    ] {
        let mut input = args();
        let index = input.iter().position(|a| *a == required).unwrap();
        input.drain(index..index + 2);
        assert!(Cli::try_parse_from(input).is_err());
    }
}
#[test]
fn copy_tree_plan_bounds_and_no_execution_overwrite_or_links() {
    assert!(
        Cli::try_parse_from(args().into_iter().chain([
            "--max-entries",
            "256",
            "--max-depth",
            "32",
            "--max-bytes",
            "268435456",
            "--timeout-ms",
            "30000"
        ]))
        .is_ok()
    );
    for bad in [
        vec!["--overwrite"],
        vec!["--execute"],
        vec!["--follow-links"],
        vec!["--max-entries", "257"],
        vec!["--max-entries", "0"],
        vec!["--max-depth", "33"],
        vec!["--max-depth", "0"],
        vec!["--max-bytes", "268435457"],
        vec!["--max-bytes", "0"],
        vec!["--timeout-ms", "0"],
        vec!["--timeout-ms", "30001"],
    ] {
        assert!(Cli::try_parse_from(args().into_iter().chain(bad)).is_err());
    }
    let mut input = args();
    input[3] = "execute";
    assert!(Cli::try_parse_from(input).is_err());
}
