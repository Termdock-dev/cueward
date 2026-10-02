# Authorized same-volume rename and move

Use installed `files rename/move` only for the requested source and new name/destination. This source revision is pending review; check installed `--help`. Observe the source and the existing destination parent with `files info`, then supply their exact versions. One absolute root bounds both relative paths.

```bash
cueward files info --root /absolute/directory --path from/report.txt
cueward files info --root /absolute/directory --path to
cueward files move --root /absolute/directory --path from/report.txt --destination to/report.txt --expected-version '<source version>' --expected-parent-version '<to version>' --dry-run
cueward files move --root /absolute/directory --path from/report.txt --destination to/report.txt --expected-version '<source version>' --expected-parent-version '<to version>'
cueward files relocation receipt --operation-id '<returned or announced ID>'
```

Rename uses `--name` with one exact leaf, keeping its existing parent; use that parent's expected-parent-version. Move uses `--destination` with the full new relative path, not an implied destination folder. Reobserve after writes. There is no overwrite, merge, conflict auto-renaming, missing-parent creation or cross-volume copy/delete fallback. Initial links, aliases/unknown alias state, placeholders/unknown availability and special files are unsupported. Available files/directories/packages and hardlinked entries can move; packages are not recursively validated. A directory cannot move into its own subtree. Exact same-path requests verify a no-op; case-only existing-path conflicts are rejected.

`--dry-run` returns rename_plan/move_plan observations including conflicts, same_filesystem and no_op. It makes no receipt, grants no execution token and does not establish native capability or reserve names. Execution requires supported guarded exclusive rename on the volume. Receipt storage need not share that volume.

The source version is checked before submission, not atomically bound to a kernel inode. External source replacement can be moved instead, including a replacement leaf symlink itself without following it. Detected identity/path differences produce incomplete evidence without rollback. Never describe this as locked paths or guaranteed selected-inode delivery. A success checks metadata/path observations, not payload hashes, recursive contents, all xattrs/ACLs or cloud sync.

Mutation JSON operations rename/move carry receipts. Exit zero requires status=completed and completion_verified=true; outer Ok alone does not suffice. Relocation_receipt lookup only reads saved evidence and can exit zero for a failed operation. Stages and null fields may predate interruption. Renamed=true means the native call succeeded, not that the requested source was verified. False means no rename by this operation, including a no-op; null is unknown.

- not_started: preflight or known native rejection, without a rename by this operation. External actors may have changed paths.
- incomplete: native success followed by verification/checkpoint failure; preserve all objects.
- uncertain: delivery/completion unknown, including timeout/lost response; actual names may already have changed.

Save the stderr operation_id before cancelling. Use `files relocation receipt --operation-id`, inspect before/actual post-observations and errors, then reobserve actual paths before deciding next action. Proposed destination_path is not an actual-location guarantee. Never automatically retry, roll back, delete, overwrite or replay the same ID. Parent SIGINT/SIGTERM/SIGKILL and timeout stop the worker but do not undo submitted rename. An interrupted parent cannot finalize evidence. Receipts remain in private ~/.cueward/operations/files-relocation directories without expiry; no payload backup is made.

Timeout-ms defaults to 10000, range 1..30000, covering the worker but not parent receipt setup/finalization. No download or Finder activation is requested. Provider_coordination remains filesystem_only_provider_state_unknown; real provider/TCC/unmount/read-only/external-volume and older macOS acceptance is incomplete. Trash, cross-volume operations, directory duplicate/recursive copy are not supported by these commands. Authorized independent batch execution uses the separate [batch command](file-batch-rename.md). Read-only [batch rename plans](file-batch-rename.md) inspect explicit proposals; never replay them as a loop of single-item writes. Authorized single-file sibling duplicate uses [file-mutations.md](file-mutations.md). Never substitute permanent deletion for trash.
