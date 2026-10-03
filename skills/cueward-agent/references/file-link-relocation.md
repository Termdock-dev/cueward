# Relocate only a selected symlink object

New `files rename/move --link-itself` capability pending review. Check installed help. Use only when the requested operation moves or renames the symlink itself, not its target. Read [relocation](file-relocation.md) for required scope, revisions, receipt statuses and interruption rules.

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

Rename uses `--name` with one exact new leaf and the current parent's version. Do not use `--follow-links`: the expected source revision is the link's own `files info` observation. Missing/false `link_itself` still rejects initial links; true requires a symlink, not an ordinary file/directory. Intermediate links, conflicts and cross-volume relocation remain rejected.

Absolute/outside, relative, broken or looping targets are reference text only. They are not resolved, opened, downloaded, queried for resources or changed. Resources are `not_applicable`. Exact inode/reference text is retained; relative references are not repaired, so moving to another parent may change their meaning. Completion does not certify target reachability or original-target continuity. Non-UTF-8 reference text is unsupported.

Plans and receipts save `link_itself=true`; legacy omission means no link permission. Require execution `status=completed` AND `completion_verified=true`. Saved lookup does not execute or reconcile anything. Preflight versions are not atomic inode guards; an external last-moment replacement can move instead and produce incomplete evidence. Independent target edits do not change this link-object contract.

Preserve the announced receipt ID, inspect actual paths on failure and reobserve before a new decision. No overwrite, retry, rollback, deletion, reference repair, payload backup or cross-volume copy/delete fallback is added. Copy/duplicate/tree copy, batch rename, trash and Finder alias resolution still do not support this flag. Synthetic fixtures and actual workers do not establish provider/TCC/external-volume/older-macOS acceptance.
