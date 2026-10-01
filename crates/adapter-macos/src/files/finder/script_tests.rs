use super::*;
use serde_json::{Value, json};
use std::time::Duration;

fn query(fixture: Value, limit: usize) -> Value {
    let directory = tempfile::tempdir().unwrap();
    let source = format!(
        "const fixture = {};\n{}\nfunction run(argv) {{ return (function(Application) {{ {}\nconst result=JSON.parse(run(argv)); result.fixture_url_reads=fixtureURLReads; return JSON.stringify(result); }})(fixtureApplication); }}",
        fixture,
        include_str!("context_fixture.js"),
        include_str!("context.js")
    );
    let script = directory.path().join("fixture.js");
    std::fs::write(&script, source).unwrap();
    let output = crate::window::process::run_with_timeout(
        Command::new("/usr/bin/osascript")
            .args(["-l", "JavaScript"])
            .arg(script)
            .arg(limit.to_string()),
        &[],
        Duration::from_secs(10),
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn finder_script_preserves_item_urls_errors_window_id_and_empty_selection() {
    let fixture = json!({"windows":1,"id":42,"location":{"url":"file:///root/"},"selection":[{"url":"file:///root/%E8%87%BA%E7%81%A3%0A%3Cexternal%3E.txt"},{"error":-1743}]});
    let result = query(fixture.clone(), 100);
    assert_eq!(result["Ok"]["front_window"]["id"], 42);
    assert_eq!(
        result["Ok"]["selection"][0]["url"],
        fixture["selection"][0]["url"]
    );
    assert_eq!(
        result["Ok"]["selection"][1]["error"]["code"],
        "permission_denied"
    );
    let result = query(json!({"windows":0,"selection":[]}), 100);
    assert!(result["Ok"]["front_window"].is_null());
    assert_eq!(result["Ok"]["selection"], json!([]));
}

#[test]
fn finder_script_preserves_virtual_window_and_top_level_permission_failures() {
    let result = query(
        json!({"windows":1,"id":1,"location":{"error":-1728},"selection":[]}),
        100,
    );
    assert_eq!(result["Ok"]["front_window"]["location"]["status"], "error");
    let result = query(json!({"permission":true}), 100);
    assert_eq!(result["Err"]["code"], "permission_denied");
    assert!(result.get("Ok").is_none());
}

#[test]
fn finder_script_rejects_oversized_selection_without_reading_item_urls() {
    let result = query(
        json!({"windows":0,"selection":[{"error":-1743},{"error":-1743}]}),
        1,
    );
    assert_eq!(result["Err"]["code"], "scan_limit");
    assert_eq!(result["fixture_url_reads"], 0);
    assert!(result.get("Ok").is_none());
    let result = query(json!({}), 501);
    assert_eq!(result["Err"]["code"], "invalid_options");
}
