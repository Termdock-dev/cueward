use super::*;
use crate::commands::Cli;
use clap::Parser;

fn args() -> Vec<&'static str> {
    vec![
        "cueward",
        "files",
        "rename-batch",
        "plan",
        "--root",
        "/tmp",
        "--entry",
        "{}",
    ]
}
#[test]
fn batch_rename_plan_parses_repeated_entries_and_requires_scope_and_entries() {
    assert!(
        Cli::try_parse_from(
            args()
                .into_iter()
                .chain(["--entry", "{}", "--timeout-ms", "30000"])
        )
        .is_ok()
    );
    for required in ["--root", "--entry"] {
        let mut args = args();
        let index = args.iter().position(|arg| *arg == required).unwrap();
        args.drain(index..index + 2);
        assert!(Cli::try_parse_from(args).is_err());
    }
}
#[test]
fn batch_rename_plan_exposes_no_execution_or_overwrite_flags() {
    for invalid in [
        vec!["--execute"],
        vec!["--overwrite"],
        vec!["--follow-links"],
        vec!["--timeout-ms", "0"],
        vec!["--timeout-ms", "30001"],
    ] {
        assert!(Cli::try_parse_from(args().into_iter().chain(invalid)).is_err());
    }
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "rename-batch",
            "execute",
            "--root",
            "/tmp"
        ])
        .is_err()
    );
}
#[test]
fn batch_rename_entries_reject_unknown_missing_wrong_type_and_oversized_json() {
    for entry in [
        "{}",
        "[]",
        "not JSON",
        r#"{"path":"a","name":"b","expected_version":"s","expected_parent_version":"p","overwrite":true}"#,
        r#"{"path":"a","name":42,"expected_version":"s","expected_parent_version":"p"}"#,
    ] {
        assert_eq!(
            parse_entries(&[entry.into()]).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
    }
    assert_eq!(
        parse_entries(&[]).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
    assert_eq!(
        parse_entries(&vec!["{}".into(); 65]).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
    assert_eq!(
        parse_entries(&["x".repeat(16385)]).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}
#[test]
fn batch_rename_entry_json_preserves_exact_names_order_and_revision_guards() {
    let entry = |path: &str, name: &str| {
        serde_json::json!({"path":path,"name":name,"expected_version":"s","expected_parent_version":"p"}).to_string()
    };
    let result =
        parse_entries(&[entry("nested/a", "臺灣\n<external>"), entry("b", "other")]).unwrap();
    assert_eq!(result[0].path, PathBuf::from("nested/a"));
    assert_eq!(result[0].name, "臺灣\n<external>");
    assert_eq!(result[1].path, PathBuf::from("b"));
    assert_eq!(result[0].expected_parent_version, "p");
}

#[test]
fn batch_execute_and_receipt_parse_without_overwrite_replay_or_plan_input() {
    let mut execute = args();
    execute[3] = "execute";
    assert!(Cli::try_parse_from(execute.clone()).is_ok());
    for flags in [
        vec!["--overwrite"],
        vec!["--resume"],
        vec!["--plan", "saved.json"],
        vec!["--timeout-ms", "0"],
    ] {
        assert!(Cli::try_parse_from(execute.clone().into_iter().chain(flags)).is_err());
    }
    assert!(
        Cli::try_parse_from([
            "cueward",
            "files",
            "rename-batch",
            "receipt",
            "--operation-id",
            "id"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["cueward", "files", "rename-batch", "receipt"]).is_err());
}
