# Scroll Read Pipeline Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `safari scroll-and-read` pipeline for infinite-scroll pages that scrolls, waits for new content, reads page text, and returns only newly discovered content across iterations.

**Architecture:** Keep the first version generic. Add a new CLI command that delegates to a Safari adapter pipeline built from small pure helpers: one helper decides whether content changed enough to count as new, another dedups newly read blocks across iterations. Use DOM text/item count polling instead of provider-specific extractor coupling so `#49` can land independently of `#48`.

**Tech Stack:** Rust, clap, serde_json, Safari JavaScript injection, cargo test

---

### Task 1: Add CLI coverage for `scroll-and-read`

**Files:**
- Modify: `crates/cli/src/main.rs`
- Test: `crates/cli/src/main.rs`

- [ ] **Step 1: Write the failing test**

Add a CLI parse test for:
`cueward safari scroll-and-read --tab x.com --profile Work --times 3`

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test cli_parses_scroll_and_read -- --exact`
Expected: FAIL because the subcommand does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Add the new `SafariAction::ScrollAndRead` clap variant with:
- `--profile`
- `--tab`
- `--times`
- `--amount`
- optional `--selector`

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test cli_parses_scroll_and_read -- --exact`
Expected: PASS

### Task 2: Lock pipeline helper behavior with pure tests

**Files:**
- Modify: `crates/adapter-macos/src/safari.rs`
- Test: `crates/adapter-macos/src/safari.rs`

- [ ] **Step 1: Write failing tests**

Add pure tests for:
- dedup keeps only unseen text blocks
- change detection treats larger DOM counts as newly loaded content
- fallback text comparison still detects new content when counts are unchanged

- [ ] **Step 2: Run tests to verify they fail**

Run:
`cargo test safari::tests::scroll_read_dedup_keeps_only_new_blocks -- --exact`
`cargo test safari::tests::scroll_read_change_detection_uses_count_or_text -- --exact`
Expected: FAIL because helpers do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Add focused helpers for:
- normalizing/deduping text blocks
- deciding whether a poll result contains newly loaded content

- [ ] **Step 4: Run tests to verify they pass**

Run the two tests above again.
Expected: PASS

### Task 3: Implement Safari adapter pipeline

**Files:**
- Modify: `crates/adapter-macos/src/safari.rs`
- Test: `crates/adapter-macos/src/safari.rs`

- [ ] **Step 1: Write the failing test**

Add a JS string-level test showing the poll script emits both a content count and page text snapshot for change detection.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test safari::tests::scroll_read_poll_script_exposes_count_and_text -- --exact`
Expected: FAIL because no such poll script exists.

- [ ] **Step 3: Write minimal implementation**

Implement:
- a Safari JS poll snippet returning item count + body/selector text
- `scroll_and_read(...)` pipeline:
  - optional tab focus
  - initial baseline read
  - repeat N times:
    - scroll
    - poll until count/text changes or timeout
    - read current content snapshot
    - return only newly discovered blocks

- [ ] **Step 4: Run targeted tests to verify it passes**

Run:
`cargo test safari::tests::scroll_read_poll_script_exposes_count_and_text -- --exact`
`cargo test safari::tests::scroll_read_dedup_keeps_only_new_blocks -- --exact`
`cargo test safari::tests::scroll_read_change_detection_uses_count_or_text -- --exact`
Expected: PASS

### Task 4: Wire CLI to adapter and verify end-to-end command surface

**Files:**
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/adapter-macos/src/safari.rs`

- [ ] **Step 1: Implement command handling**

Wire `SafariAction::ScrollAndRead` to the new adapter function and print structured JSON via `print_external`.

- [ ] **Step 2: Run focused verification**

Run:
`cargo test -p cueward-adapter-macos safari`
`cargo test -p cueward cli_parses_scroll_and_read -- --exact`

- [ ] **Step 3: Run broader package verification**

Run:
`cargo test -p cueward-adapter-macos`

- [ ] **Step 4: Review diff**

Run:
`git diff -- crates/cli/src/main.rs crates/adapter-macos/src/safari.rs docs/plans/2026-04-12-scroll-read-pipeline.md`
