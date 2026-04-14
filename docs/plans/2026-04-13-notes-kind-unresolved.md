# Notes Kind And Unresolved Fallback Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a typed `AttachmentSegment.kind` contract and ensure notes with attachment placeholders always emit attachment segments, even when no resolver can materialize a file.

**Architecture:** Extend the core cue schema with an additive `AttachmentKind` enum, then teach the notes attachment pipeline to emit `kind = image` for the existing image path and `kind = unresolved` for unresolved placeholders. Keep the current content replacement and OCR flow intact, but stop returning an empty `attachment_segments` array when placeholders remain unresolved.

**Tech Stack:** Rust Edition 2024, serde, chrono, cargo test

---

### Task 1: Add the core typed attachment contract

**Files:**
- Modify: `crates/core/src/cue.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write failing schema tests**

Cover:
- serializing an image segment includes `kind: "image"`
- deserializing legacy attachment JSON without `kind` still succeeds with a safe default

- [ ] **Step 2: Run the targeted core tests and verify they fail**

Run:
`cargo test -p cueward-core cue -- --nocapture`

Expected: FAIL because `AttachmentKind` and `AttachmentSegment.kind` do not exist yet.

- [ ] **Step 3: Add the minimal core schema**

Implement:
- `AttachmentKind` enum with at least `image` and `unresolved` plus the planned future variants from `#9`
- additive `kind` field on `AttachmentSegment`
- serde defaults so legacy payloads remain readable

- [ ] **Step 4: Re-run the targeted core tests**

Run:
`cargo test -p cueward-core cue -- --nocapture`

Expected: PASS

### Task 2: Align the existing image pipeline with `kind = image`

**Files:**
- Modify: `crates/adapter-macos/src/notes/attachments/mod.rs`

- [ ] **Step 1: Write failing attachment tests for typed image segments**

Cover:
- image segments emitted by `build_attachment_segments()` carry `kind = image`
- the existing OCR-related fields still serialize as before

- [ ] **Step 2: Run the targeted notes attachment tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos notes::attachments::tests::build_attachment_segments -- --nocapture`

Expected: FAIL because `AttachmentSegment.kind` is not populated yet.

- [ ] **Step 3: Implement the minimal image-path alignment**

Update the notes attachment builder to set `kind = image` for the current materialized image attachments.

- [ ] **Step 4: Re-run the targeted attachment tests**

Run:
`cargo test -p cueward-adapter-macos notes::attachments::tests::build_attachment_segments -- --nocapture`

Expected: PASS

### Task 3: Add unresolved fallback for unmatched placeholders

**Files:**
- Modify: `crates/adapter-macos/src/notes/attachments/mod.rs`
- Test: `crates/adapter-macos/src/notes/attachments/mod.rs`

- [ ] **Step 1: Write failing unresolved-fallback tests**

Cover:
- a note with placeholder text and no matching media note emits unresolved segments instead of an empty `attachment_segments`
- a note with matching media metadata but no materialized attachments still emits unresolved segments

- [ ] **Step 2: Run the targeted fallback tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos unresolved -- --nocapture`

Expected: FAIL because unresolved fallback does not exist yet.

- [ ] **Step 3: Implement the fallback**

Implement minimal unresolved segment creation:
- `kind = unresolved`
- `has_ocr = false`
- no fake path or OCR text
- one segment per unresolved placeholder

- [ ] **Step 4: Re-run the targeted fallback tests**

Run:
`cargo test -p cueward-adapter-macos unresolved -- --nocapture`

Expected: PASS

### Task 4: Final verification

**Files:**
- Modify: `docs/plans/2026-04-13-notes-kind-unresolved.md` only if implementation drifted

- [ ] **Step 1: Run full verification**

Run:
`cargo test`
`cargo build --release`

Expected: PASS

- [ ] **Step 2: Confirm the acceptance criteria**

Check:
- image attachments now emit `kind = image`
- placeholder-only notes no longer end with empty `attachment_segments`
- unresolved samples are visible for future resolver work

- [ ] **Step 3: Prepare the handoff**

Document in the eventual PR summary that follow-up work should continue in:
- `#78` structured URL-like attachments

Plan complete and saved to `docs/plans/2026-04-13-notes-kind-unresolved.md`. Ready to execute.
