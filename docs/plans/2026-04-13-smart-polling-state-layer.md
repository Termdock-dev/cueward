# Smart Polling State Layer Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a persistent scan state layer with backoff, 2-strike delete detection, and content pre-filtering for Reddit and X targets.

**Architecture:** Extend `cueward_core::State` with generic `scan_targets` metadata, then implement a small shared scan-state helper in `adapter-macos` that provider modules can use. Reddit and X will build canonical target URLs, apply provider-specific filters, compute fingerprints, and return scan-aware envelopes such as `fresh`, `unchanged`, `skipped`, `warning`, or `deleted`.

**Tech Stack:** Rust, chrono, serde, serde_json, cargo test

---

### Task 1: Extend persistent core state schema

**Files:**
- Modify: `crates/core/src/state.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write failing state round-trip tests**

Add tests for:
- loading legacy state without `scan_targets`
- saving/loading `scan_targets`
- updating a stored scan target entry

- [ ] **Step 2: Run the targeted tests and verify they fail**

Run:
`cargo test -p cueward-core state -- --nocapture`

Expected: FAIL because `scan_targets` and helper accessors do not exist yet.

- [ ] **Step 3: Add the minimal schema**

Implement:
- `ScanTargetState`
- `#[serde(default)] scan_targets: HashMap<String, ScanTargetState>`
- helper methods for get/set of scan target entries
- export `ScanTargetState` from `cueward_core`

- [ ] **Step 4: Re-run the targeted tests**

Run:
`cargo test -p cueward-core state -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/state.rs crates/core/src/lib.rs
git commit -m "feat: add persistent scan target state"
```

### Task 2: Add shared scan-state helper module in adapter-macos

**Files:**
- Modify: `crates/adapter-macos/src/lib.rs`
- Create: `crates/adapter-macos/src/scan_state.rs`

- [ ] **Step 1: Write failing pure tests in `scan_state.rs`**

Cover:
- scan key building from `provider + target_url`
- skip/backoff decisions
- 2-strike delete transitions
- fingerprint stability
- bot/deleted author filtering
- age cutoff filtering

- [ ] **Step 2: Run the targeted tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos scan_state::tests -- --nocapture`

Expected: FAIL because the module does not exist yet.

- [ ] **Step 3: Add the shared helper implementation**

Implement:
- `ScanStatus`
- `ScanEnvelope<T>`
- `build_scan_key()`
- `should_skip_scan()`
- `record_success()`
- `record_not_found()`
- `fingerprint_json()`
- small pre-filter helpers
- state load/save wrapper that does not fail the provider result on save errors

- [ ] **Step 4: Re-run targeted scan-state tests**

Run:
`cargo test -p cueward-adapter-macos scan_state::tests -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/adapter-macos/src/lib.rs crates/adapter-macos/src/scan_state.rs
git commit -m "feat: add shared scan state helpers"
```

### Task 3: Apply scan state to Reddit provider

**Files:**
- Modify: `crates/adapter-macos/src/reddit.rs`
- Modify: `crates/adapter-macos/src/reddit/tests.rs`
- Modify: `crates/cli/src/commands/reddit.rs`

- [ ] **Step 1: Add failing tests for scan-aware Reddit behavior**

Cover:
- repeated identical feed/search results become `unchanged`
- stale target can become `skipped`
- single 404 on `post` becomes `warning`
- second consecutive 404 on the same post becomes `deleted`
- comment pre-filter removes deleted/bot/too-old/too-short comments

- [ ] **Step 2: Run targeted tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos reddit::tests -- --nocapture`

Expected: FAIL because Reddit still returns raw provider results.

- [ ] **Step 3: Implement Reddit scan integration**

Make Reddit commands:
- load/update persistent scan state
- use canonical `old.reddit.com` target URLs
- fingerprint feed/search post lists
- fingerprint filtered top-level comments for `post`
- return `ScanEnvelope<...>` instead of raw provider data

Update CLI dispatch to print the scan envelope cleanly.

- [ ] **Step 4: Re-run targeted verification**

Run:
`cargo test -p cueward-adapter-macos reddit::tests -- --nocapture`
`cargo test -p cueward-cli reddit_tests -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/adapter-macos/src/reddit.rs crates/adapter-macos/src/reddit/tests.rs crates/cli/src/commands/reddit.rs
git commit -m "feat: add reddit scan state tracking"
```

### Task 4: Apply scan state to X provider

**Files:**
- Modify: `crates/adapter-macos/src/safari/social/x.rs`
- Modify: `crates/cli/src/commands/safari_ai/x.rs`

- [ ] **Step 1: Write failing X-focused tests**

Cover:
- X post filtering removes deleted/bot/too-old/too-short items
- canonical target URL selection for feed/search/read
- repeated identical results become `unchanged`
- `x read` empty result uses 2-strike deleted flow

- [ ] **Step 2: Run targeted tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos x_ -- --nocapture`

Expected: FAIL because X still returns raw `Vec<SocialFeedPost>`.

- [ ] **Step 3: Integrate scan state into X**

Implement:
- canonical URLs for feed/home, search, and read targets
- provider-specific pre-filter for `SocialFeedPost`
- fingerprinting after filtering
- not-found handling for `x read` when the targeted post returns no parsed items
- scan envelope output for X CLI commands

- [ ] **Step 4: Re-run targeted verification**

Run:
`cargo test -p cueward-adapter-macos x_ -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/adapter-macos/src/safari/social/x.rs crates/cli/src/commands/safari_ai/x.rs
git commit -m "feat: add x scan state tracking"
```

### Task 5: Full verification and docs touch-up

**Files:**
- Modify: `docs/specs/2026-04-13-smart-polling-state-layer.md` (only if implementation drifted)
- Modify: `README.md` (only if command output semantics need a note)

- [ ] **Step 1: Run full verification**

Run:
`cargo build --release`
`cargo test`

Expected: PASS

- [ ] **Step 2: Review docs for semantic drift**

If output/status semantics changed in a user-visible way, add a short note to the README.

- [ ] **Step 3: Commit any docs alignment**

```bash
git add README.md docs/specs/2026-04-13-smart-polling-state-layer.md
git commit -m "docs: align smart polling docs"
```
