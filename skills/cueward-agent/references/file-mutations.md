# Authorized no-overwrite creation, copy and duplicate

Check installed files mkdir/copy/duplicate/receipt help. Select only the requested root, source and exact new destination. Revisions do not authorize a write. Observe source and the destination existing parent with files info; provide expected-version (copy/duplicate) and expected-parent-version. Reobserve after writes.

```bash
cueward files mkdir --root /absolute/directory --path Reports --expected-parent-version '<parent version>'
cueward files copy --root /absolute/directory --path Project --destination Archive/Project-copy --expected-version '<source version>' --expected-parent-version '<Archive version>'
cueward files duplicate --root /absolute/directory --path Owned.app --name 'Owned copy.app' --expected-version '<source version>' --expected-parent-version '<parent version>'
cueward files receipt --operation-id '<returned or stderr-announced ID>'
```

Mkdir creates one empty directory (0700 subject to umask), not missing parents. Copy/duplicate accept available files, directory/package trees, opaque Finder aliases and exact symlink references. Broken/outside references are valid; targets are never followed, queried or repaired. Duplicate takes one new sibling filename, not a destination path or an automatic Finder suffix. Explicitly selecting a package selects its contents. Hardlinked source files produce independent copies. Unknown type/availability, special entries/permissions and compressed sources are refused.

Never overwrite/merge/auto-rename a conflict, traverse ancestor links or copy a directory into its own subtree. Generic copy/duplicate is bounded to 256 nodes including root, depth 32 and aggregate file/reference bytes within max-bytes (1..256 MiB/default 64 MiB). Per-node xattrs are bounded to 64 KiB names/4 MiB values; receipt limits can refuse smaller trees with long paths/evidence. Worker timeout is 1..30000 ms/default 10000, excluding parent setup/finalization. Detailed [copy-tree proposals](file-copy-tree.md) and [execution](file-copy-tree-execution.md) retain separate 256/64-node and package-flag rules; do not replay a saved plan as a command loop.

Objects are copied and verified privately before one exclusive beneath-root publication, with no later payload writes. Different filesystems use a private `.cueward-stage-<operation-id>` under the destination parent; receipts remain in ~/.cueward/operations/files/<id>. Staging/backups can remain indefinitely after failure. Never delete them without separate cleanup authorization. Content/tree/reference text, mode/mtime and bounded xattrs are verified, not ACLs/owner/group/birthtime/flags/hardlink relationships/provider synchronization or App installation.

Require status=completed and completion_verified=true. Outer Ok and receipt lookup success do not prove delivery. Not_started means known rejection/no requested publication; private data may remain. Incomplete means a known destination followed by failure. Uncertain means delivery cannot be established, including cancellation; a destination can exist before its checkpoint. Preserve announced ID, receipt, staging and actual source/destination; inspect before a new decision. Never automatically retry, rollback, delete a partial destination or replay the operation ID.

Parent death/deadlines stop local workers, not submitted filesystem/provider work. Checks are sequential observations, not atomic snapshot/inode CAS or isolation. Provider/TCC/read-only/unmount/older-macOS behavior is not established by local fixture success. Other authorized operations use [relocation](file-relocation.md), [tags](file-tags.md), [confirmed trash](file-trash-execution.md) and [backup restoration](file-trash-restore.md). Permanent deletion is excluded.
