# ChatGPT Save Images Stability Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `safari ai --provider chatgpt save-images` wait until generated images are actually loaded before extraction, so downloads stop failing due to premature capture.

**Architecture:** Extend the ChatGPT image poll payload with a minimal loaded-state signal derived from DOM image readiness, then gate completion on at least one loaded image instead of any matching image node. Keep the existing command shape and save flow intact so this stays a small regression fix rather than a broader pipeline rewrite.

**Tech Stack:** Rust, serde_json, Safari JavaScript injection, cargo test

---

### Task 1: Add regression coverage for loaded image state

**Files:**
- Modify: `crates/adapter-macos/src/safari.rs`
- Test: `crates/adapter-macos/src/safari.rs`

- [ ] **Step 1: Write the failing test**

Add a parser-level regression test showing that image payloads can carry `loaded: false/true`, and that the parsed image preserves the flag.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test parse_chatgpt_image_payload_reads_loaded_state -- --exact`
Expected: FAIL because `SafariAiImage` does not yet include the loaded field.

- [ ] **Step 3: Write minimal implementation**

Extend `SafariAiImage` and `parse_chatgpt_image_payload()` to read and preserve the boolean loaded flag, defaulting to `false` when absent.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test parse_chatgpt_image_payload_reads_loaded_state -- --exact`
Expected: PASS

### Task 2: Tighten poll completion condition

**Files:**
- Modify: `crates/adapter-macos/src/safari.rs`
- Test: `crates/adapter-macos/src/safari.rs`

- [ ] **Step 1: Write the failing test**

Add a small pure-function test covering the intended completion rule:
- `complete + unloaded images` should not count as ready
- `complete + at least one loaded image` should count as ready

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test chatgpt_image_result_requires_loaded_images_to_be_ready -- --exact`
Expected: FAIL because the readiness helper does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Introduce a tiny helper for readiness and use it inside `poll_chatgpt_images()`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test chatgpt_image_result_requires_loaded_images_to_be_ready -- --exact`
Expected: PASS

### Task 3: Emit loaded state from Safari DOM polling

**Files:**
- Modify: `crates/adapter-macos/src/safari.rs`
- Test: `crates/adapter-macos/src/safari.rs`

- [ ] **Step 1: Write the failing test**

Add a string-level test for `chatgpt_image_list_js()` that asserts the script includes image completion signals such as `img.complete` and/or `naturalWidth`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test chatgpt_image_list_script_includes_loaded_signal -- --exact`
Expected: FAIL because the current script only emits URL and dimensions.

- [ ] **Step 3: Write minimal implementation**

Update the JS payload to emit `loaded` based on image readiness, and dedupe while preserving that field.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test chatgpt_image_list_script_includes_loaded_signal -- --exact`
Expected: PASS

### Task 4: Verify targeted regression path

**Files:**
- Modify: none
- Test: existing adapter tests

- [ ] **Step 1: Run focused tests**

Run:
`cargo test parse_chatgpt_image_payload_reads_loaded_state -- --exact`
`cargo test chatgpt_image_result_requires_loaded_images_to_be_ready -- --exact`
`cargo test chatgpt_image_list_script_includes_loaded_signal -- --exact`

Expected: all PASS

- [ ] **Step 2: Run broader adapter verification**

Run: `cargo test safari`
Expected: existing Safari adapter tests stay green

- [ ] **Step 3: Review diff**

Run: `git diff -- crates/adapter-macos/src/safari.rs docs/plans/2026-04-12-chatgpt-save-images-stability.md`

- [ ] **Step 4: Commit**

```bash
git add crates/adapter-macos/src/safari.rs docs/plans/2026-04-12-chatgpt-save-images-stability.md
git commit -m "fix: wait for loaded ChatGPT images before saving"
```
