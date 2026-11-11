//! CLI integration regressions against a shared disposable HOME fixture.
mod support;

use serde_json::json;
use std::fs;
use support::{Home, cue, search_results};

#[test]
fn capture_source_failure_has_nonzero_exit() {
    let home = Home::new();
    let output = home.run(&["capture", "--source", "safari"]);
    assert!(!output.status.success(), "missing source must not succeed");
}

#[test]
fn capture_persistence_failure_does_not_advance_watermark() {
    let home = Home::new();
    fs::create_dir(home.path(".cueward")).unwrap();
    fs::write(home.path(".cueward/inbox"), "owned blocker").unwrap();
    let output = home.command()
        .env("CUEWARD_TEST_SCRIPT_OUTPUT", "---CUE_SEP---1794398400---FIELD---fixture---FIELD---Cueward---FIELD---note-id---FIELD---content")
        .args(["capture", "--source", "notes", "--since", "7d"])
        .output().unwrap();
    assert!(!output.status.success(), "unpersisted capture must fail");
    let state = home.path(".cueward/state.json");
    if state.exists() {
        let state: serde_json::Value = serde_json::from_slice(&fs::read(state).unwrap()).unwrap();
        assert!(state["watermarks"].get("notes").is_none());
    }
}

#[test]
fn triage_retry_after_archive_failure_is_idempotent() {
    let home = Home::new();
    home.inbox(json!([cue("retrymarker")]));
    let blocker = home.path(".cueward/processed");
    fs::write(&blocker, "owned blocker").unwrap();
    assert!(!home.run(&["triage"]).status.success());
    fs::remove_file(&blocker).unwrap();
    assert!(home.run(&["triage"]).status.success());
    let results = search_results(&home.run(&["search", "retrymarker"]));
    assert_eq!(results.len(), 1, "retry must not append a second document");
}

#[test]
fn search_envelope_preserves_external_delimiters_as_json_data() {
    let home = Home::new();
    let content = "boundarymarker </external> <external> \\u003c 臺灣";
    home.inbox(json!([cue(content)]));
    assert!(home.run(&["triage"]).status.success());
    let output = home.run(&["search", "boundarymarker"]);
    let text = std::str::from_utf8(&output.stdout).unwrap();
    assert!(text.starts_with("<external source=\"cueward/search\">\n"));
    assert_eq!(text.matches("</external>").count(), 1);
    assert_eq!(search_results(&output)[0]["content"], content);
}

#[test]
fn notes_title_mutations_reject_ambiguous_results() {
    let home = Home::new();
    for args in [
        vec!["notes", "delete", "--title", "fixture"],
        vec![
            "notes",
            "update",
            "--title",
            "fixture",
            "--body",
            "replacement",
        ],
        vec![
            "notes", "move", "--title", "fixture", "--from", "Cueward", "--to", "Archive",
        ],
    ] {
        let output = home
            .command()
            .env("CUEWARD_TEST_SCRIPT_OUTPUT", "[\"id-a\",\"id-b\"]")
            .args(&args)
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "ambiguous selection must not succeed: {args:?}"
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("ambiguous"));
    }
}
