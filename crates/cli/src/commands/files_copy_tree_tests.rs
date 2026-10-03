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
fn copy_tree_plan_bounds_and_no_overwrite_or_links() {
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
    assert!(Cli::try_parse_from(input).is_ok());
}

#[test]
fn tree_execute_parsing_has_separate_entry_bound_and_no_replay_or_overwrite() {
    let mut input = args();
    input[3] = "execute";
    assert!(Cli::try_parse_from(input.clone()).is_ok());
    for flags in [
        vec!["--max-entries", "65"],
        vec!["--max-entries", "0"],
        vec!["--overwrite"],
        vec!["--plan", "saved.json"],
        vec!["--resume"],
        vec!["--follow-links"],
        vec!["--timeout-ms", "0"],
    ] {
        assert!(Cli::try_parse_from(input.clone().into_iter().chain(flags)).is_err());
    }
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "copy-tree",
            "receipt",
            "--operation-id",
            "id"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["cueward", "files", "copy-tree", "receipt"]).is_err());
}

#[test]
fn copy_tree_package_inclusion_is_explicit_for_plan_and_execute_not_receipt_or_single_file_copy() {
    // Both commands retain all existing required guards and bounds.
    for action in ["plan", "execute"] {
        let mut input = args();
        input[3] = action;
        assert!(Cli::try_parse_from(input.clone()).is_ok());
        assert!(Cli::try_parse_from(input.into_iter().chain(["--include-packages"])).is_ok());
    }
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "copy-tree",
            "receipt",
            "--operation-id",
            "id",
            "--include-packages"
        ])
        .is_err()
    );
    let mut single_file = args();
    single_file.remove(2);
    single_file[2] = "copy";
    assert!(Cli::try_parse_from(single_file.clone()).is_ok());
    assert!(Cli::try_parse_from(single_file.into_iter().chain(["--include-packages"])).is_err());
}
