# Safari Bookmarks CRUD Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `cueward safari bookmarks` list/search/add/delete commands on macOS, backed by Safari `Bookmarks.plist`, with full folder-path support and `title + url` fingerprint semantics inside a folder.

**Architecture:** Keep the product surface under the existing `safari` command tree, but follow the repo rule that new Safari-adjacent features must not grow `safari.rs` with feature logic. Implement bookmark plist parsing and CRUD in a new sibling module `crates/adapter-macos/src/bookmarks.rs`, expose it from `lib.rs`, and let the CLI dispatch to that module directly. Preserve existing bookmark plist fields by using a lossless raw plist mutation path for add/delete, and keep bookmarks behind the shared Safari session guard so they serialize with other Safari commands.

**Tech Stack:** Rust 2024, `clap`, `serde`/`serde_json`, new `plist` crate, `tempfile`, `cargo test`

---

## Chunk 1: CLI Contract And Bookmark Read Path

### Task 1: Add Safari bookmarks CLI parse coverage

**Files:**
- Modify: `crates/cli/src/main.rs`

- [ ] **Step 1: Write the failing parse tests**

Add tests near the existing Safari CLI parse tests for:

```rust
#[test]
fn cli_parses_safari_bookmarks_list_with_folder() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "bookmarks",
        "list",
        "--folder",
        "Work/AI Tools",
    ])
    .expect("parse safari bookmarks list");

    match cli.command {
        Command::Safari {
            action: SafariAction::Bookmarks {
                action: SafariBookmarksAction::List { folder },
            },
        } => assert_eq!(folder.as_deref(), Some("Work/AI Tools")),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_safari_bookmarks_delete_with_title_url_and_folder() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "bookmarks",
        "delete",
        "--title",
        "Claude",
        "--url",
        "https://claude.ai",
        "--folder",
        "Work/AI Tools",
    ])
    .expect("parse safari bookmarks delete");

    match cli.command {
        Command::Safari {
            action: SafariAction::Bookmarks {
                action: SafariBookmarksAction::Delete { title, url, folder },
            },
        } => {
            assert_eq!(title, "Claude");
            assert_eq!(url, "https://claude.ai");
            assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
        }
        _ => panic!("unexpected command"),
    }
}
```

Also cover:

- `bookmarks search <query> --folder ...`
- `bookmarks add --title ... --url ... --folder ...`

- [ ] **Step 2: Run the parse tests to verify they fail**

Run: `cargo test -p cueward-cli cli_parses_safari_bookmarks -- --nocapture`

Expected: FAIL because `SafariAction::Bookmarks` and `SafariBookmarksAction` do not exist yet.

- [ ] **Step 3: Add the minimal CLI enums to make parsing compile**

In `crates/cli/src/main.rs`:

- add `SafariBookmarksAction` as a nested `#[derive(Subcommand)]` enum
- add `SafariAction::Bookmarks { action: SafariBookmarksAction }`
- keep the new enum limited to:
  - `List { folder: Option<String> }`
  - `Search { query: String, folder: Option<String> }`
  - `Add { title: String, url: String, folder: Option<String> }`
  - `Delete { title: String, url: String, folder: Option<String> }`

Do not add dispatch logic yet. This task is only about making the CLI contract concrete and testable.

- [ ] **Step 4: Run the parse tests to verify they pass**

Run: `cargo test -p cueward-cli cli_parses_safari_bookmarks -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "test: define safari bookmarks cli contract"
```

### Task 2: Add bookmark read/search tests and module scaffolding

**Files:**
- Modify: `crates/adapter-macos/Cargo.toml`
- Modify: `crates/adapter-macos/src/lib.rs`
- Create: `crates/adapter-macos/src/bookmarks.rs`

- [ ] **Step 1: Write the failing adapter tests for read-only behavior**

Create `crates/adapter-macos/src/bookmarks.rs` with tests that use inline sample plist content plus `tempfile::tempdir()` where file I/O is needed.

Add tests for:

```rust
#[test]
fn bookmarks_lists_direct_children_for_folder_path() {
    let tree = sample_bookmarks_tree();
    let items = list_items_in_folder(&tree, Some("Work/AI Tools")).expect("list folder");

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title.as_deref(), Some("Claude"));
    assert_eq!(items[1].title.as_deref(), Some("Grok"));
}

#[test]
fn bookmarks_searches_recursively_from_folder_path() {
    let tree = sample_bookmarks_tree();
    let items = search_items(&tree, "claude", Some("Work")).expect("search");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].folder_path, "Work/AI Tools");
    assert_eq!(items[0].url.as_deref(), Some("https://claude.ai"));
}
```

Also add:

- invalid folder path test
- folder path parser test for `Work/AI Tools`
- root listing test for direct children only

- [ ] **Step 2: Run the adapter tests to verify they fail**

Run: `cargo test -p cueward-adapter-macos bookmarks_ -- --nocapture`

Expected: FAIL because the new `bookmarks` module, helpers, and `plist` dependency are missing.

- [ ] **Step 3: Add minimal module wiring and read/search implementation**

In `crates/adapter-macos/Cargo.toml`:

- add `plist = "1"`

In `crates/adapter-macos/src/lib.rs`:

- add `pub mod bookmarks;`

In `crates/adapter-macos/src/bookmarks.rs`:

- define typed plist/domain structs needed for read/search only
- parse folder paths into non-empty segments
- recursively walk `Children`
- implement folder lookup from root and `list` / `search` helpers
- keep URL values opaque strings; do not add canonicalization logic
- support `--profile` as a root-folder prefix, so `--profile Work --folder "Projects/AI Tools"` resolves to `Work/Projects/AI Tools`

Prefer a small read model:

- `BookmarkTree`
- `BookmarkFolder`
- `BookmarkEntry`
- `SafariBookmarkItem`
- `SafariBookmarksListResult`
- `SafariBookmarksSearchResult`

Keep the module small. If `bookmarks.rs` starts approaching the repo's 500-line cap, split pure helpers into a second sibling file in the same PR rather than stuffing everything into one file.

- [ ] **Step 4: Run the adapter tests to verify they pass**

Run: `cargo test -p cueward-adapter-macos bookmarks_ -- --nocapture`

Expected: PASS for the new read/search/path tests

- [ ] **Step 5: Commit**

```bash
git add crates/adapter-macos/Cargo.toml crates/adapter-macos/src/lib.rs crates/adapter-macos/src/bookmarks.rs
git commit -m "feat: add safari bookmark read model"
```

## Chunk 2: Bookmark Mutation And CLI Wiring

### Task 3: Add add/delete mutation tests and implement fingerprint rules

**Files:**
- Modify: `crates/adapter-macos/src/bookmarks.rs`

- [ ] **Step 1: Write the failing mutation tests**

Add tests for the approved fingerprint semantics:

```rust
#[test]
fn bookmarks_add_rejects_duplicate_title_and_url_in_same_folder() {
    let mut tree = sample_bookmarks_tree();

    let err = add_bookmark(
        &mut tree,
        Some("Work/AI Tools"),
        "Claude",
        "https://claude.ai",
    )
    .expect_err("duplicate should fail");

    assert!(err.to_string().contains("duplicate bookmark"));
}

#[test]
fn bookmarks_add_allows_same_title_with_different_url() {
    let mut tree = sample_bookmarks_tree();

    add_bookmark(
        &mut tree,
        Some("Work/AI Tools"),
        "Claude",
        "https://example.com/claude-alt",
    )
    .expect("same title, different url is allowed");
}

#[test]
fn bookmarks_delete_matches_title_and_url() {
    let mut tree = sample_bookmarks_tree();

    let deleted = delete_bookmark(
        &mut tree,
        Some("Work/AI Tools"),
        "Claude",
        "https://claude.ai",
    )
    .expect("delete");

    assert_eq!(deleted.title, "Claude");
    assert_eq!(deleted.url.as_deref(), Some("https://claude.ai"));
}
```

Also add:

- delete not found test
- delete conflict test when the fixture intentionally contains duplicate `title + url`
- round-trip file test that reads a temp plist, mutates it, writes it back, and reloads it

- [ ] **Step 2: Run the mutation tests to verify they fail**

Run: `cargo test -p cueward-adapter-macos bookmarks_add_ -- --nocapture`

Run: `cargo test -p cueward-adapter-macos bookmarks_delete_ -- --nocapture`

Expected: FAIL because mutation helpers and write-back logic do not exist yet.

- [ ] **Step 3: Implement minimal add/delete and plist persistence**

In `crates/adapter-macos/src/bookmarks.rs`:

- add public functions:
  - `list_bookmarks(folder: Option<&str>) -> Result<SafariBookmarksListResult, MacosError>`
  - `search_bookmarks(query: &str, folder: Option<&str>) -> Result<SafariBookmarksSearchResult, MacosError>`
  - `add_bookmark_cli(title: &str, url: &str, folder: Option<&str>) -> Result<SafariBookmarkMutationResult, MacosError>`
  - `delete_bookmark_cli(title: &str, url: &str, folder: Option<&str>) -> Result<SafariBookmarkMutationResult, MacosError>`
- read `~/Library/Safari/Bookmarks.plist`
- for `add/delete`, mutate the raw plist tree in place and preserve untouched fields
- tolerate empty folder nodes that omit the `Children` key by treating them as empty lists
- write the updated plist back once on success
- treat duplicate `title + url` entries in one folder as data conflict for delete
- return precise `MacosError::Other(...)` messages matching the spec:
  - `bookmarks plist not found`
  - `invalid folder path`
  - `duplicate bookmark`
  - `bookmark not found`
  - `bookmark data conflict`
  - `plist decode failed`
  - `plist write failed`

Do not add folder creation or recursive list output in this task.

- [ ] **Step 4: Run the mutation tests to verify they pass**

Run: `cargo test -p cueward-adapter-macos bookmarks_add_ -- --nocapture`

Run: `cargo test -p cueward-adapter-macos bookmarks_delete_ -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/adapter-macos/src/bookmarks.rs
git commit -m "feat: implement safari bookmark mutations"
```

### Task 4: Wire CLI dispatch, JSON output, and docs

**Files:**
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/adapter-macos/src/lib.rs`
- Modify: `README.md`

- [ ] **Step 1: Add the failing glue assertions where the codebase already has seams**

The current CLI architecture inlines dispatch inside `main.rs`, so there is no clean test seam for command execution without a larger refactor. Keep scope tight:

- rely on the existing parse tests from Task 1
- rely on the adapter behavior tests from Tasks 2-3
- use compile + targeted package tests as the glue verification step

Before changing dispatch, add or update any small enum/result tests that help the compiler catch mismatch early, but do not introduce a new abstraction layer just to test dispatch.

- [ ] **Step 2: Wire the new command variants through the CLI**

In `crates/cli/src/main.rs`:

- handle `SafariAction::Bookmarks`
- dispatch directly to `cueward_adapter_macos::bookmarks::*`
- build the effective target path from `--profile` + `--folder`
- for `list`, `search`, `add`, and `delete`, print the pretty JSON payload with `print_external(...)` because bookmark title/url are untrusted external content
- keep stderr status text short and consistent with existing Safari commands

In `crates/adapter-macos/src/lib.rs`:

- keep `pub mod bookmarks;` exported for CLI use

In the shared Safari guard module:

- extract the existing session lock types/helpers from `safari.rs` into a shared internal module
- keep `safari.rs` and `bookmarks.rs` both behind the same `with_safari_session()`

In `README.md`:

- add a `Safari bookmarks` subsection near the existing Safari command examples
- document the exact four commands plus the `Work/AI Tools` folder path example
- show delete with both `--title` and `--url`

- [ ] **Step 3: Run targeted verification**

Run: `cargo test -p cueward-cli cli_parses_safari_bookmarks -- --nocapture`

Run: `cargo test -p cueward-adapter-macos bookmarks_ -- --nocapture`

Run: `cargo test -p cueward-cli`

Run: `cargo test -p cueward-adapter-macos`

Expected:

- new bookmarks parse tests PASS
- adapter bookmark tests PASS
- full package test suites stay green

- [ ] **Step 4: Run formatter**

Run: `cargo fmt`

Expected: no diff after formatting, or only mechanical formatting changes

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/main.rs crates/adapter-macos/src/lib.rs README.md
git commit -m "feat: add safari bookmarks cli"
```

## Final Verification

- [ ] Run: `cargo test -p cueward-cli`
- [ ] Run: `cargo test -p cueward-adapter-macos`
- [ ] Skim `README.md` examples against the implemented CLI flags
- [ ] Manually inspect that `docs/specs/2026-04-13-safari-bookmarks-crud.md` and the implementation still match

## Notes For Execution

- Use `@test-driven-development` discipline while executing: tests first, then minimal implementation.
- Do not expand scope into folder creation, rename, move, or recursive list output.
- Keep bookmark logic in `crates/adapter-macos/src/bookmarks.rs`; do not add new feature code to `safari.rs`.
- It is acceptable to move the shared Safari guard into a dedicated internal module if that is required to keep `bookmarks` serialized with other Safari commands.
- Prefer inline plist fixtures in Rust tests over adding a new fixtures directory unless the inline sample becomes unreadable.
