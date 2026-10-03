# Explicit package-directory copying

`files copy-tree plan/execute --include-packages` adds bounded package-directory copying for [#42](https://github.com/Termdock-dev/cueward/issues/42). This source capability was merged in PR #63; check installed help. It reuses the [tree planning](files-copy-tree.md), [verified tree execution](files-copy-tree-execution.md) and typed receipt contracts. Without this option, package directories are still recorded as unsupported and their contents are not traversed.

## Select the contents explicitly

Use the option only when the requested copy includes package contents. A package's Finder presentation does not make it an opaque file to this engine: every descendant consumes the existing entry/depth/byte budgets and must pass validation. An explicit sibling destination expresses duplication; `files duplicate` remains regular-file-only and no automatic Finder-style name is selected.

```bash
cueward files metadata --root /Users/me/Documents --path Owned.app
cueward files info --root /Users/me/Documents --path Owned.app
cueward files info --root /Users/me/Documents --path .
cueward files copy-tree plan --root /Users/me/Documents --path Owned.app \
  --destination Owned-copy.app --expected-version '<source version>' \
  --expected-parent-version '<parent version>' --include-packages
# Reobserve before an independently authorized execution.
cueward files copy-tree execute --root /Users/me/Documents --path Owned.app \
  --destination Owned-copy.app --expected-version '<fresh source version>' \
  --expected-parent-version '<fresh parent version>' --include-packages
cueward files copy-tree receipt --operation-id '<announced UUID>'
```

Root is absolute; source and exact destination are scoped relative paths. The parent must already exist and the destination must be absent, including broken-link conflicts. Keep the extension when the intended result should have the same extension-based Finder package presentation. Cueward does not choose an extension or guarantee Finder's resulting classification from arbitrary destination names.

## What the option permits

The option permits traversal only for a directory whose native `resources.is_package` is `available/true`; known ordinary directories remain supported. It applies to the selected package and known nested packages under an ordinary or package source. Unknown, unsupported, not-applicable or errored package state remains a blocker even with the option. The flag does not turn unknown state into false, resolve aliases or follow links.

Every descendant still has to be an available supported regular file or directory with known non-alias state and supported metadata. Symlinks, Finder aliases, special files, placeholders/unknown availability, special permissions, compressed sources and unsupported attributes remain blockers. Execution refuses the whole request before staging if the fresh plan has any blocker or incomplete enumeration; it does not publish a package with unsupported children silently omitted. Many real `.app` bundles contain symlinks and therefore remain unsupported in this slice. A suffix alone is not proof of a known package role.

Planning defaults to 256 entries; execution defaults to and accepts at most 64, including every package directory and descendant. Depth is 16 by default/range 1..32; aggregate file bytes default to 64 MiB/range 1..256 MiB; whole-worker timeout defaults to 10000 ms/range 1..30000. The existing request, plan, xattr and receipt limits are unchanged. Opting in does not increase any bound, imply available space or promise that a typical application bundle will fit.

## Verification and saved evidence

There is no new writer or native application-installation path. The worker freshly replans, holds no-follow descriptors, builds all nodes in private same-volume staging, verifies file SHA-256/bytes, file/directory xattrs, permissions/mtime and exact child sets, and publishes once with guarded exclusive root-relative rename. It then reads the published tree back and checks source/root/parent consistency. Source payloads are retained. No overwrite, merge, alias/link following, cross-volume copy/delete, activation, launch, retry, rollback or cleanup is added.

Operations stay `copy_tree_plan`, `copy_tree` and `copy_tree_receipt`. A plan's `execution_supported` is still false. Execution exit zero requires `status=completed` and `completion_verified=true`; outer `Ok` and successful saved lookup alone are not completion. Package-aware requests and receipts carry `include_packages=true`. Missing or false values mean the legacy package-skipping policy; false is omitted on serialization to preserve old request/receipt shapes. Non-boolean values are rejected. A historical receipt without the field never gains package permission. Compatibility is one-way: older CLI builds using strict unknown-field parsing cannot read opt-in receipts containing `include_packages=true`; preserve the field and use a compatible binary, never strip permission evidence or replay the operation.

Ordinary tree interruption/retention rules apply unchanged. Workers are claimed once, monitor parent lifetime and stop on deadline. Private partial copies can remain; a receipt can lag actual publication. Inspect before a new decision and do not automatically retry or delete retained user data. Sequential revision/descriptor checks are not an atomic snapshot or namespace lock.

This verifies a filesystem content copy, not an installable/runnable application, valid code signature/notarization, license transfer, document semantic integrity, Finder refresh or provider synchronization. Ownership, ACLs, birth/access times, flags, sparse layout and hardlink relationships are not promised preserved. `provider_coordination=filesystem_only_provider_state_unknown`; no implicit download. Copies of document packages can capture sequentially observed bytes without an application-level save transaction.

## Verification scope and remaining work

Owned macOS 27.0.1 fixtures exercise known `.app` directories, selected and nested packages, ordinary-source trees containing packages, Unicode/newline names, hidden nodes, independent identities, unchanged source bytes, mtime/permissions and file/directory xattrs. Tests also cover default pruning, unknown native package states, links, conflicts, unchanged budgets, post-staging edits, saved opt-in/legacy JSON, actual CLI/worker envelopes and lookup, and parent SIGINT/SIGTERM/SIGKILL/deadline after private copy progress. No personal application or document package is launched, edited or removed.

These synthetic filesystem fixtures do not establish real signed App bundle installation, arbitrary document-format correctness, provider/TCC/read-only/unmount/external-volume behavior or older macOS support. #42 remains open for broader package/link/metadata support, cross-volume operations, existing-tag edits, broader trash/recovery and remaining platform acceptance. Permanent deletion is excluded.
