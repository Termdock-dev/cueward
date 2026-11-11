//! CLI integration regressions against a shared disposable HOME fixture.
mod support;

use serde_json::json;
use std::fs;
use support::{Home, cue, notes_output, search_results};

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
    let output = home
        .command()
        .env(
            "CUEWARD_TEST_SCRIPT_OUTPUT",
            notes_output(&[("note-id", "content")]),
        )
        .args(["capture", "--source", "notes", "--since", "7d"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "unpersisted capture must fail");
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to save inbox"));
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

#[test]
fn repeated_captures_and_same_second_source_records_are_deduplicated_by_identity() {
    let home = Home::new();
    let data = notes_output(&[("id-a", "replaymarker"), ("id-b", "replaymarker")]);
    for _ in 0..2 {
        let output = home
            .command()
            .env("CUEWARD_TEST_SCRIPT_OUTPUT", &data)
            .args(["capture", "--source", "notes", "--incremental"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(home.run(&["triage"]).status.success());
    assert_eq!(
        search_results(&home.run(&["search", "replaymarker"])).len(),
        2
    );
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(home.path(".cueward/state.json")).unwrap()).unwrap();
    assert_eq!(state["watermarks"]["notes"], "2026-11-11T12:00:00Z");
}

#[test]
fn partial_capture_keeps_successful_data_and_reports_failure() {
    let home = Home::new();
    let output = home
        .command()
        .env(
            "CUEWARD_TEST_SCRIPT_OUTPUT",
            notes_output(&[("id", "partialmarker")]),
        )
        .args(["capture"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("safari:") && stderr.contains("messages:"));
    assert_eq!(
        fs::read_dir(home.path(".cueward/inbox")).unwrap().count(),
        1
    );
    assert!(home.run(&["triage"]).status.success());
    assert_eq!(
        search_results(&home.run(&["search", "partialmarker"])).len(),
        1
    );
}

#[test]
fn empty_search_is_a_wrapped_empty_array() {
    let home = Home::new();
    let output = home.run(&["search", "missing"]);
    assert!(output.status.success());
    assert!(
        std::str::from_utf8(&output.stdout)
            .unwrap()
            .starts_with("<external source=\"cueward/search\">\n")
    );
    assert!(search_results(&output).is_empty());
}

#[test]
fn checkpoint_failure_keeps_saved_batch_and_can_be_retried() {
    let home = Home::new();
    fs::create_dir(home.path(".cueward")).unwrap();
    let state =
        json!({"watermarks":{"notes":"2026-11-10T12:00:00Z"}, "scan_targets":{}}).to_string();
    fs::write(home.path(".cueward/state.json"), &state).unwrap();
    let blocker = home.path(".cueward/state.lock");
    fs::create_dir(&blocker).unwrap();
    let run = || {
        home.command()
            .env(
                "CUEWARD_TEST_SCRIPT_OUTPUT",
                notes_output(&[("id-a", "checkpointmarker")]),
            )
            .args(["capture", "--source", "notes", "--incremental"])
            .output()
            .unwrap()
    };
    let first = run();
    assert!(!first.status.success());
    assert!(String::from_utf8_lossy(&first.stderr).contains("failed to save state"));
    assert_eq!(
        fs::read_to_string(home.path(".cueward/state.json")).unwrap(),
        state
    );
    assert_eq!(
        fs::read_dir(home.path(".cueward/inbox")).unwrap().count(),
        1
    );
    fs::remove_dir(blocker).unwrap();
    assert!(run().status.success());
    assert!(home.run(&["triage"]).status.success());
    assert_eq!(
        search_results(&home.run(&["search", "checkpointmarker"])).len(),
        1
    );
}

#[test]
fn corrupt_state_or_inbox_is_reported_without_discarding_the_original() {
    let home = Home::new();
    fs::create_dir(home.path(".cueward")).unwrap();
    let state = home.path(".cueward/state.json");
    fs::write(&state, "corrupted owned state").unwrap();
    let output = home.run(&["capture", "--source", "notes"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to load state"));
    assert_eq!(fs::read_to_string(&state).unwrap(), "corrupted owned state");
    home.inbox(json!([]));
    let batch = home.path(".cueward/inbox/batch.json");
    fs::write(&batch, "corrupted owned batch").unwrap();
    assert!(!home.run(&["triage"]).status.success());
    assert_eq!(fs::read_to_string(batch).unwrap(), "corrupted owned batch");
}

#[test]
fn triage_never_overwrites_a_different_archived_batch() {
    let home = Home::new();
    home.inbox(json!([cue("archivemarker")]));
    fs::create_dir(home.path(".cueward/processed")).unwrap();
    let archived = home.path(".cueward/processed/batch.json");
    fs::write(&archived, "different original archive").unwrap();
    assert!(!home.run(&["triage"]).status.success());
    assert_eq!(
        fs::read_to_string(archived).unwrap(),
        "different original archive"
    );
    assert!(home.path(".cueward/inbox/batch.json").exists());
}

#[test]
fn notes_attachment_read_failure_preserves_checkpoint_for_incremental_retry() {
    let home = Home::new();
    fs::create_dir(home.path(".cueward")).unwrap();
    let previous =
        json!({"watermarks":{"notes":"2026-11-10T12:00:00Z"}, "scan_targets":{}}).to_string();
    fs::write(home.path(".cueward/state.json"), &previous).unwrap();
    let records = json!([
        {"timestamp":"2026-11-11T12:00:00Z", "note_id":"older-attachment", "folder":"Cueward", "title":"fixture", "content":"older \u{fffc}"},
        {"timestamp":"2026-11-11T18:00:00Z", "note_id":"newer-note", "folder":"Cueward", "title":"fixture", "content":"newer plain text"}
    ]);
    // The owned HOME intentionally has no NoteStore. Native Notes/JXA is the shared fake boundary.
    let output = home
        .command()
        .env("CUEWARD_TEST_SCRIPT_OUTPUT", records.to_string())
        .args(["capture", "--source", "notes", "--incremental"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "attachment read failure must not be reported as a complete Notes source"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("NoteStore.sqlite"));
    assert_eq!(
        fs::read_to_string(home.path(".cueward/state.json")).unwrap(),
        previous
    );
    assert!(!home.path(".cueward/inbox").exists());
}

#[test]
fn literal_attachment_label_in_plain_text_does_not_require_notestore() {
    let home = Home::new();
    let output = home
        .command()
        .env(
            "CUEWARD_TEST_SCRIPT_OUTPUT",
            notes_output(&[("plain", "[Attachment] is only quoted text")]),
        )
        .args(["capture", "--source", "notes", "--incremental"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !home
            .path("Library/Group Containers/group.com.apple.notes/NoteStore.sqlite")
            .exists()
    );
}
