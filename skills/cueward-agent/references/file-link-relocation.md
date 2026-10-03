# Relocate only a selected symlink object

`files rename/move --link-itself` was merged in PR #64. Check installed help. Use only when the requested operation moves or renames the symlink itself, not its target. Read [relocation](file-relocation.md) for required scope, revisions, receipt statuses and interruption rules.

```bash
cueward files info --root /absolute/directory --path from/link
cueward files info --root /absolute/directory --path to
cueward files move --root /absolute/directory --path from/link \
  --destination to/link --expected-version '<link version>' \
  --expected-parent-version '<to version>' --link-itself --dry-run
# Reobserve before an authorized execution; omit dry-run only for that requested write.
cueward files move --root /absolute/directory --path from/link \
  --destination to/link --expected-version '<fresh link version>' \
  --expected-parent-version '<fresh to version>' --link-itself
cueward files relocation receipt --operation-id '<announced UUID>'
```

Rename uses `--name` with one exact new leaf and the current parent's version. Do not use `--follow-links`: the expected source revision is the link's own `files info` observation. Missing/false `link_itself` still rejects initial links; true requires a symlink, not an ordinary file/directory. Intermediate links and conflicts are rejected; different-volume move uses verified copy and private original retention.

Absolute/outside, relative, broken or looping targets are reference text only. They are not resolved, opened, downloaded, queried for resources or changed. Resources are `not_applicable`. Same-volume inode and exact reference text are retained; relative references are not repaired, so moving to another parent may change their meaning. Completion does not certify target reachability or original-target continuity. Non-UTF-8 reference text is unsupported.

Plans and receipts save `link_itself=true`; legacy omission means no link permission. Require execution `status=completed` AND `completion_verified=true`. Saved lookup does not execute or reconcile anything. Preflight versions are not atomic inode guards; an external last-moment replacement can move instead and produce incomplete evidence. Independent target edits do not change this link-object contract.

Older pre-#64 receipt readers silently ignore `link_itself=true` and omit it from typed output, including `before.request`, while still exiting successfully. That output is incomplete authorization evidence; omission does not prove the original request had no opt-in. Lookup does not rewrite the stored receipt. Read with the same binary whose `files rename/move --help` supports `--link-itself`, and preserve the raw `receipt_path` JSON unchanged. Do not strip fields, move receipt namespaces or replay to work around version skew. New readers accept legacy omission as false; old workers still reject initially selected links. Older-binary lookup is not fail-closed.

Preserve the announced receipt ID and inspect actual paths on failure. Same-volume link relocation preserves the original inode; cross-volume move creates a verified independent reference object then retains the original privately. No target repair, overwrite, retry or automatic cleanup is performed. Copy/duplicate/tree copy and trash also support link objects; --link-itself is relocation-only. Batch rename uses [per-entry permission](file-batch-link-rename.md), merged in PR #65.