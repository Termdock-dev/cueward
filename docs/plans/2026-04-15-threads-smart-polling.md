# Threads Smart Polling Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add scan state, backoff, and pre-filtering to Threads feed scans without adding post/search support.

**Architecture:** Reuse the existing `scan_state` helper and follow the X provider pattern, but scope the target to the Threads feed only. Return `ScanEnvelope<Vec<SocialFeedPost>>` from the adapter and surface the envelope unchanged through the CLI.

**Tech Stack:** Rust, chrono, serde, cargo test

---

### Task 1: Add failing Threads smart-polling tests

**Files:**
- Modify: `crates/adapter-macos/src/safari/social/threads.rs`
- Modify: `crates/cli/src/commands/safari_ai_tests.rs`

- [ ] **Step 1: Write failing adapter tests**

Cover:
- repeated identical filtered feed results become `unchanged`
- backoff returns `skipped`
- short / bot-like / too-old posts are filtered out

- [ ] **Step 2: Write failing CLI parse coverage if needed**

Only add CLI coverage if command shape changes need explicit assertion.

- [ ] **Step 3: Run targeted tests to verify failure**

Run:
`cargo test -p cueward-adapter-macos threads -- --nocapture`

Expected: FAIL because Threads still returns raw `Vec<SocialFeedPost>`.

### Task 2: Implement Threads scan envelope flow

**Files:**
- Modify: `crates/adapter-macos/src/safari/social/threads.rs`
- Modify: `crates/cli/src/commands/safari_ai/threads.rs`

- [ ] **Step 1: Add minimal adapter implementation**

Implement:
- canonical target URL for Threads feed
- scan state load / skip / record success flow
- provider-specific Threads filtering helper
- fingerprint after filtering
- `ScanEnvelope<Vec<SocialFeedPost>>` return type

- [ ] **Step 2: Update CLI output**

Print the envelope JSON under the same external source name.

- [ ] **Step 3: Re-run targeted tests**

Run:
`cargo test -p cueward-adapter-macos threads -- --nocapture`
`cargo test -p cueward-cli safari_ai_tests -- --nocapture`

Expected: PASS

### Task 3: Full verification

**Files:**
- Modify: `README.md` only if user-visible semantics need a note

- [ ] **Step 1: Run full verification**

Run:
`cargo build --release`
`cargo test`

Expected: PASS

- [ ] **Step 2: Commit**

```bash
git add docs/specs/2026-04-15-threads-smart-polling.md docs/plans/2026-04-15-threads-smart-polling.md crates/adapter-macos/src/safari/social/threads.rs crates/cli/src/commands/safari_ai/threads.rs crates/cli/src/commands/safari_ai_tests.rs
git commit -m "feat: add threads smart polling"
```
