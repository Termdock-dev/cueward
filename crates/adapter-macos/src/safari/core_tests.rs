use super::core::{
    scroll_read_detects_new_content, scroll_read_new_content_blocks, scroll_read_poll_js,
    scroll_read_snapshot_blocks,
};
use super::types::SafariScrollReadSnapshot;

#[test]
fn scroll_read_dedup_keeps_only_new_blocks() {
    let seen = vec!["alpha".to_string(), "beta".to_string()];
    let content = "alpha\n\nbeta\n\ngamma\n\ndelta\n\ngamma";

    let blocks = scroll_read_new_content_blocks(content, &seen);

    assert_eq!(blocks, vec!["gamma".to_string(), "delta".to_string()]);
}

#[test]
fn scroll_read_change_detection_uses_count_or_text() {
    assert!(scroll_read_detects_new_content("same", 2, "same", 3));
    assert!(scroll_read_detects_new_content("before", 2, "after", 2));
    assert!(!scroll_read_detects_new_content("same", 2, "same", 2));
}

#[test]
fn scroll_read_snapshot_blocks_prefers_structured_blocks() {
    let snapshot = SafariScrollReadSnapshot {
        item_count: 2,
        content: "alpha\n\nbeta\n\ngamma".to_string(),
        blocks: vec!["alpha".to_string(), "gamma".to_string()],
    };

    let blocks = scroll_read_snapshot_blocks(&snapshot, &["alpha".to_string()]);

    assert_eq!(blocks, vec!["gamma".to_string()]);
}

#[test]
fn scroll_read_poll_script_exposes_count_and_text() {
    let script = scroll_read_poll_js(Some("main"));

    assert!(script.contains("item_count"));
    assert!(script.contains("content"));
    assert!(script.contains("blocks"));
    assert!(script.contains("querySelectorAll(\"article\")"));
    assert!(script.contains("document.querySelector(\"main\")"));
}

#[test]
fn scroll_read_poll_script_uses_visible_items() {
    let script = scroll_read_poll_js(None);

    assert!(script.contains("getBoundingClientRect"));
    assert!(script.contains("window.innerHeight"));
    assert!(script.contains("rect.bottom > 0"));
}

#[test]
fn scroll_read_poll_script_has_heuristic_fallback() {
    let script = scroll_read_poll_js(None);

    assert!(script.contains("itemNodes.length === 0"));
    assert!(script.contains("querySelectorAll(\"div\")"));
}

#[test]
fn scroll_read_fingerprint_dedup_handles_suffix_changes() {
    let long_prefix = "a]".repeat(61);
    let seen = vec![format!("{long_prefix}ORIGINAL_SUFFIX")];
    let content = format!("{long_prefix}UPDATED_SUFFIX\n\ngenuinely new block");

    let blocks = scroll_read_new_content_blocks(&content, &seen);

    assert_eq!(blocks, vec!["genuinely new block".to_string()]);
}
