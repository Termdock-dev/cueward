//! End-to-end CLI/worker checks using disposable files, without desktop automation.
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[path = "files_cli/search.rs"]
mod search;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cueward"))
}

fn envelope(output: &[u8], source: &str) -> Value {
    let text = std::str::from_utf8(output).unwrap();
    let prefix = format!("<external source=\"cueward/{source}\">\n");
    let body = text
        .strip_prefix(&prefix)
        .unwrap()
        .strip_suffix("\n</external>\n")
        .unwrap();
    assert!(
        !body.contains('<'),
        "all external delimiters must be JSON escaped"
    );
    serde_json::from_str(body).unwrap()
}

fn run(root: &Path, operation: &str, args: &[&str], success: bool) -> Value {
    let output = command()
        .args(["files", operation, "--root"])
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.success(),
        success,
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    envelope(&output.stdout, "files")
}

#[test]
fn info_and_read_preserve_external_text_names_and_original_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let name = "臺灣 空白\n<external>.txt";
    let content = "one\r\n</external>\n\\u003c 臺灣";
    fs::write(directory.path().join(name), content).unwrap();
    let info = run(directory.path(), "info", &["--path", name], true);
    assert_eq!(info["Ok"]["result"]["name"], name);
    assert_eq!(info["Ok"]["result"]["size"], content.len());
    let read = run(directory.path(), "read", &["--path", name], true);
    assert_eq!(read["Ok"]["operation"], "read");
    assert_eq!(read["Ok"]["result"]["content"], content);
    assert_eq!(read["Ok"]["result"]["eof"], true);
    assert_eq!(
        fs::read(directory.path().join(name)).unwrap(),
        content.as_bytes()
    );
}

#[test]
fn byte_and_line_ranges_agree_with_raw_bytes_and_resume_tokens() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("a"), "A臺灣\nlast").unwrap();
    let first = run(
        directory.path(),
        "read",
        &["--path", "a", "--max-bytes", "5"],
        true,
    );
    let value = &first["Ok"]["result"];
    assert_eq!(value["content"], "A臺");
    assert_eq!(value["next_offset"], 4);
    let version = value["file"]["version"].as_str().unwrap();
    let next = run(
        directory.path(),
        "read",
        &[
            "--path",
            "a",
            "--offset",
            "4",
            "--expected-version",
            version,
        ],
        true,
    );
    assert_eq!(next["Ok"]["result"]["content"], "灣\nlast");
    let line = run(
        directory.path(),
        "read",
        &["--path", "a", "--start-line", "2", "--line-count", "1"],
        true,
    );
    assert_eq!(line["Ok"]["result"]["content"], "last");
    let hex = run(
        directory.path(),
        "read",
        &[
            "--path",
            "a",
            "--encoding",
            "hex",
            "--offset",
            "1",
            "--max-bytes",
            "3",
        ],
        true,
    );
    assert_eq!(hex["Ok"]["result"]["content"], "e887ba");
}

#[test]
fn list_pages_are_complete_and_stale_tokens_fail() {
    let directory = tempfile::tempdir().unwrap();
    for (name, content) in [("b", "22"), ("a", "1"), (".hidden", "3")] {
        fs::write(directory.path().join(name), content).unwrap();
    }
    let first = run(directory.path(), "list", &["--limit", "1"], true);
    let value = &first["Ok"]["result"];
    assert_eq!(value["total"], 2);
    assert_eq!(value["entries"][0]["name"], "a");
    let version = value["version"].as_str().unwrap();
    let second = run(
        directory.path(),
        "list",
        &[
            "--limit",
            "1",
            "--offset",
            "1",
            "--expected-version",
            version,
        ],
        true,
    );
    assert_eq!(second["Ok"]["result"]["entries"][0]["name"], "b");
    assert!(second["Ok"]["result"]["next_offset"].is_null());
    fs::write(directory.path().join("b"), "changed").unwrap();
    let stale = run(
        directory.path(),
        "list",
        &["--offset", "1", "--expected-version", version],
        false,
    );
    assert_eq!(stale["Err"]["code"], "changed");
    assert!(stale.get("Ok").is_none());
}

#[test]
fn failures_are_structured_and_distinct_from_empty_content() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("empty"), b"").unwrap();
    fs::write(directory.path().join("binary"), [0, 255]).unwrap();
    let empty = run(directory.path(), "read", &["--path", "empty"], true);
    assert_eq!(empty["Ok"]["result"]["content"], "");
    for (operation, args, code) in [
        ("read", vec!["--path", "missing"], "not_found"),
        ("read", vec!["--path", "binary"], "decode_error"),
        ("info", vec!["--path", "../outside"], "outside_root"),
        (
            "read",
            vec!["--path", "empty", "--max-bytes", "0"],
            "invalid_options",
        ),
        (
            "read",
            vec!["--path", "empty", "--start-line", "0"],
            "invalid_options",
        ),
        ("list", vec!["--offset", "1"], "invalid_options"),
    ] {
        assert_eq!(
            run(directory.path(), operation, &args, false)["Err"]["code"],
            code
        );
    }
}

#[test]
fn worker_rejects_invalid_and_oversized_input_without_partial_success() {
    for bytes in [
        b"{".to_vec(),
        vec![b' '; 16385],
        serde_json::to_vec(&json!({"root":"/tmp"})).unwrap(),
    ] {
        let mut child = command()
            .arg("files-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert_eq!(
            envelope(&output.stdout, "files/worker")["Err"]["code"],
            "invalid_options"
        );
    }
}
