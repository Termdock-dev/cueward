# Reddit JSON API Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a new top-level `cueward reddit` command that reads Reddit via `old.reddit.com/*.json` without Safari automation.

**Architecture:** Keep Reddit as a standalone read-only provider. Put HTTP + JSON parsing in `crates/adapter-macos/src/reddit.rs`, expose it from `adapter-macos`, and add a dedicated CLI command module at `crates/cli/src/commands/reddit.rs`. Use `ureq` for synchronous requests, normalize subreddit / post URL inputs up front, and return structured JSON wrapped with `print_external()`.

**Tech Stack:** Rust, clap, serde, serde_json, ureq, cargo test

---

### Task 1: Add CLI command surface for `cueward reddit`

**Files:**
- Modify: `crates/cli/src/commands/mod.rs`
- Create: `crates/cli/src/commands/reddit.rs`
- Create: `crates/cli/src/commands/reddit_tests.rs`

- [ ] **Step 1: Write the failing CLI parse tests**

Add tests for:
- `cueward reddit feed rust`
- `cueward reddit feed r/rust --limit 50`
- `cueward reddit post https://www.reddit.com/r/rust/comments/abc123/example_title/`
- `cueward reddit search "async rust"`
- `cueward reddit search "async rust" --subreddit r/rust --limit 25`

- [ ] **Step 2: Run the new tests and verify they fail**

Run:
`cargo test reddit_tests -- --nocapture`

Expected: FAIL because the `reddit` command does not exist yet.

- [ ] **Step 3: Add the minimal clap surface**

Implement:
- `Command::Reddit { action: RedditAction }`
- `RedditAction::{Feed, Post, Search}`
- `dispatch()` stub in `commands/reddit.rs`

- [ ] **Step 4: Run the CLI tests again**

Run:
`cargo test reddit_tests -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/commands/mod.rs crates/cli/src/commands/reddit.rs crates/cli/src/commands/reddit_tests.rs
git commit -m "feat: add reddit cli surface"
```

### Task 2: Implement Reddit adapter core with pure parser tests

**Files:**
- Modify: `crates/adapter-macos/src/lib.rs`
- Modify: `crates/adapter-macos/Cargo.toml`
- Create: `crates/adapter-macos/src/reddit.rs`

- [ ] **Step 1: Write failing pure tests in `reddit.rs`**

Cover:
- subreddit normalization (`rust`, `r/rust`, invalid cases)
- post URL normalization to `old.reddit.com/.../.json?limit=500`
- feed URL builder
- search URL builder
- feed JSON parsing
- post JSON parsing with top-level comments only

- [ ] **Step 2: Run the targeted tests and verify they fail**

Run:
`cargo test reddit::tests -- --nocapture`

Expected: FAIL because the module and helpers do not exist yet.

- [ ] **Step 3: Add the adapter implementation**

Implement in `crates/adapter-macos/src/reddit.rs`:
- public result structs for feed / post / search
- internal JSON helpers for listing / thing parsing
- `normalize_subreddit()`
- `normalize_post_url()`
- `build_feed_url()`
- `build_search_url()`
- `fetch_json()`
- `feed()`
- `post()`
- `search()`

Add `ureq` to `crates/adapter-macos/Cargo.toml`.
Expose the module with `pub mod reddit;` in `crates/adapter-macos/src/lib.rs`.

- [ ] **Step 4: Run the targeted adapter tests**

Run:
`cargo test -p cueward-adapter-macos reddit::tests -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/adapter-macos/Cargo.toml crates/adapter-macos/src/lib.rs crates/adapter-macos/src/reddit.rs
git commit -m "feat: add reddit json adapter"
```

### Task 3: Wire CLI dispatch to adapter and external output

**Files:**
- Modify: `crates/cli/src/commands/reddit.rs`

- [ ] **Step 1: Replace CLI stubs with real dispatch**

Wire each action to:
- `cueward_adapter_macos::reddit::feed`
- `cueward_adapter_macos::reddit::post`
- `cueward_adapter_macos::reddit::search`

Wrap outputs with `print_external()` using:
- `reddit/feed`
- `reddit/post`
- `reddit/search`

- [ ] **Step 2: Run focused CLI package tests**

Run:
`cargo test -p cueward-cli reddit_tests -- --nocapture`

Expected: PASS

- [ ] **Step 3: Run full repo verification**

Run:
`cargo build --release`
`cargo test`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/commands/reddit.rs
git commit -m "feat: wire reddit json workflows"
```

### Task 4: Update user-facing docs

**Files:**
- Modify: `README.md`
- Modify: `skills/cueward-agent/SKILL.md`

- [ ] **Step 1: Add README usage examples**

Document:
- `cueward reddit feed`
- `cueward reddit post`
- `cueward reddit search`

- [ ] **Step 2: Update skill documentation**

Add Reddit command coverage so the installed skill reflects the new CLI surface.

- [ ] **Step 3: Re-run verification**

Run:
`cargo test`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add README.md skills/cueward-agent/SKILL.md
git commit -m "docs: document reddit json commands"
```
