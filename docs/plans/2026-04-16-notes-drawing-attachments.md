# Notes Drawing Attachments Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect Apple Notes drawing/sketch attachments and emit typed `drawing` segments instead of unresolved placeholders.

**Architecture:** Add a minimal drawing note/attachment path in the Notes DB layer, then thread it through the existing attachment enrichment pipeline before unresolved fallback. Keep the first batch classification-only: no full drawing export, no OCR, no new CLI surface.

**Tech Stack:** Rust, rusqlite, serde, cargo test

---

### Task 1: Lock down expected drawing behavior in tests

**Files:**
- Modify: `crates/core/src/cue.rs`
- Modify: `crates/adapter-macos/src/notes/attachments/tests.rs`

- [ ] **Step 1: Add a core serialization test for drawing**

Verify `AttachmentSegment { kind: Drawing }` serializes as `"drawing"`.

- [ ] **Step 2: Add a failing notes attachment pipeline test**

Cover:
- note with attachment placeholder
- matched drawing note row
- emitted segment is `kind = drawing`
- unresolved fallback does not trigger for that slot

- [ ] **Step 3: Run targeted tests and verify failure**

Run:
`cargo test -p cueward-core cue::tests -- --nocapture`
`cargo test -p cueward-adapter-macos notes::attachments::tests -- --nocapture`

Expected: FAIL because notes pipeline has no drawing resolver yet.

### Task 2: Add minimal drawing structures and DB detection

**Files:**
- Modify: `crates/adapter-macos/src/notes/mod.rs`
- Modify: `crates/adapter-macos/src/notes/db/mod.rs`
- Create: `crates/adapter-macos/src/notes/db/drawing.rs`

- [ ] **Step 1: Define drawing note/attachment structs**

Add minimal structs parallel to existing note attachment families.

- [ ] **Step 2: Write failing DB-level tests**

Cover:
- drawing attachment row shape maps into a typed drawing attachment
- non-drawing rows do not leak into drawing loader

- [ ] **Step 3: Implement minimal drawing loader**

Implement:
- query shape
- stable signal detection for drawing/sketch rows
- note grouping by timestamp/title

- [ ] **Step 4: Re-run targeted DB tests**

Run:
`cargo test -p cueward-adapter-macos drawing -- --nocapture`

Expected: PASS

### Task 3: Thread drawing through attachment enrichment

**Files:**
- Modify: `crates/adapter-macos/src/notes/attachments/mod.rs`
- Create: `crates/adapter-macos/src/notes/attachments/drawing.rs`

- [ ] **Step 1: Add failing drawing resolver tests**

Cover:
- drawing labels / segments count aligns with placeholders
- drawing resolver runs before unresolved fallback
- mixed attachments preserve correct indices

- [ ] **Step 2: Implement drawing resolver**

Implement:
- note matching
- optional labels helper
- `AttachmentSegment { kind: Drawing }`
- pipeline integration before unresolved fallback

- [ ] **Step 3: Re-run targeted attachment tests**

Run:
`cargo test -p cueward-adapter-macos notes::attachments::tests -- --nocapture`

Expected: PASS

### Task 4: Full verification and issue hygiene

**Files:**
- Modify: `docs/specs/2026-04-16-notes-drawing-attachments.md` only if implementation drifted

- [ ] **Step 1: Run full verification**

Run:
`cargo build --release`
`cargo test`

Expected: PASS

- [ ] **Step 2: Close or update issue references if needed**

If implementation fully satisfies #16, close it with exact merge reference.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/cue.rs crates/adapter-macos/src/notes/mod.rs crates/adapter-macos/src/notes/db/mod.rs crates/adapter-macos/src/notes/db/drawing.rs crates/adapter-macos/src/notes/attachments/mod.rs crates/adapter-macos/src/notes/attachments/drawing.rs crates/adapter-macos/src/notes/attachments/tests.rs docs/specs/2026-04-16-notes-drawing-attachments.md docs/plans/2026-04-16-notes-drawing-attachments.md
git commit -m "feat: classify notes drawing attachments"
```
