# Explicit symlink objects in independent batch rename

This #42 slice extends [batch planning](files-batch-rename.md) and [independent execution](files-batch-execution.md) with per-entry symlink-object selection. Per-entry link permission was merged in PR #65. Check installed `files rename-batch plan/execute --help` for `link_itself` before using it; single-item `--link-itself` support does not establish batch support.

## Request only the selected link objects

Observe each source and its existing parent using `files info`, without `--follow-links`. Use the link's own version. Serialize one entry per explicitly selected source and exact new sibling name:

```bash
cueward files info --root /absolute/directory --path link
cueward files info --root /absolute/directory --path report.txt
cueward files info --root /absolute/directory --path .
cueward files rename-batch plan --root /absolute/directory \
  --entry '{"path":"link","name":"renamed-link","expected_version":"<link version>","expected_parent_version":"<root version>","link_itself":true}' \
  --entry '{"path":"report.txt","name":"report final.txt","expected_version":"<file version>","expected_parent_version":"<root version>"}'
# Reobserve if anything changed; execute only the user's authorized batch.
cueward files rename-batch execute --root /absolute/directory \
  --entry '{"path":"link","name":"renamed-link","expected_version":"<fresh link version>","expected_parent_version":"<fresh root version>","link_itself":true}' \
  --entry '{"path":"report.txt","name":"report final.txt","expected_version":"<fresh file version>","expected_parent_version":"<fresh root version>"}'
cueward files rename-batch receipt --operation-id '<announced aggregate UUID>'
cueward files relocation receipt --operation-id '<item child UUID>'
```

`link_itself` is a JSON boolean on each entry, not a global `--link-itself` flag. Missing/false retains default source-link rejection. True requires that entry to be a leaf symlink; ordinary files/directories with true are refused. One invalid entry blocks the entire preflight, including otherwise valid ordinary entries. Use a JSON serializer, not string concatenation of external names.

Entries may mix ordinary files, permitted link objects and same-path no-ops under different existing parents. The original limits remain: 1..64 entries, complete serialized request at most 16 KiB, whole-worker timeout 1..30000 ms/default 10000. No larger budget, target repair, automatic suffix, overwrite, chain/swap ordering or ancestor-link traversal is added.

## The reference is not another selection

The [single-item link-object contract](files-link-relocation.md) applies to every permitted child. Absolute/outside, relative, broken or looping references are opaque UTF-8 text, never resolved, opened, queried for target resources, downloaded or edited. Non-UTF-8 text is refused. Link resources are `not_applicable`. The native rename retains the symlink inode and exact reference text. Batch rename keeps each link's parent, but other explicitly selected entries can change what a relative reference resolves to; references are never repaired and completion does not establish reachability or original-target continuity. An unselected target's independent edits do not invalidate a link-object guard.

Whole-batch preflight, canonical-root anchoring, fresh remaining-item plans, verified parent/root revision advancement and one-use supervised child receipts are reused. Replanning retains each entry's explicit permission. Existing conflicts, dependencies and unavailable-source refusals still stop all later submissions. Final aggregate checks permit only each opted-in destination leaf, compare its observed identity/version and exact reference text, and recheck source absence except no-ops, parents and root. They do not follow targets or relax intermediate-link refusal.

Planning remains read-only with `execution_supported=false`; outer Ok/exit zero can contain item errors or conflicts. Execution exit zero requires aggregate `status=completed` and `completion_verified=true`. Individual verified children do not override a later aggregate failure. Interruption after native progress can leave uncertain aggregate evidence and untouched unallocated entries, without undoing submitted renames. Inspect aggregate and child evidence plus actual paths; never retry, resume, roll back or delete automatically.

## Saved evidence and CLI version skew

Aggregate original entries and child effective requests retain `link_itself=true`. Missing/false defaults to false and false is omitted, preserving legacy non-link JSON. Wrong types and unknown fields are rejected. New readers accept older aggregate receipts without this field. Older batch entry decoders already deny unknown fields, so they reject new opt-in entries and aggregate lookup rather than dropping permission. This does not rewrite the saved receipt or add a schema/namespace migration.

Child evidence retains the existing relocation schema. Pre-#64 relocation readers can silently omit link permission while exiting successfully, as documented in [receipt reader compatibility](files-link-relocation.md#receipt-readers-across-cli-revisions). Read both aggregate and child receipts with the same binary whose batch help exposes `link_itself` and whose rename/move help exposes `--link-itself`. Preserve raw receipt JSON unchanged; never strip fields, move namespaces or replay an operation to work around an older reader.

## Verification and remaining limits

Owned macOS 27.0.1 tests cover default refusal, explicit boolean parsing/legacy omission, mixed files/links, broken/outside/relative references, nested parents, no-ops, inode/reference retention, unchanged outside target bytes/revisions, saved aggregate/child lookups, one-use refusal, stale/non-link/conflict/dependency preflight refusal, pending and completed link replacements, independent target edits, and parent SIGINT/SIGTERM/SIGKILL after the first actual native rename. Actual CLI/worker tests check escaped Unicode/newline names, permission propagation and no child allocation on failed whole-batch preflight. Only disposable owned files and exact newly allocated receipt directories are cleaned up. A release CLI/worker probe on 2026-10-03 passed help discovery, read-only mixed plans, default whole-batch refusal, mixed native rename/no-op, inode/reference preservation, unchanged outside target bytes/revision and aggregate/child saved lookup.

Current copy/duplicate, different-volume moves, existing-tag edits and directory/link trash/restore are described in [file mutations](files-mutations.md), [relocation](files-relocation.md), [tags](files-tags.md) and [trash](files-trash-execution.md). File Provider/TCC/read-only/unmount, arbitrary external volumes and older macOS remain unverified. Permanent deletion is excluded.