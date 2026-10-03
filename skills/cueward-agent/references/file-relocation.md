# Authorized same-volume rename and move

Use installed `files rename/move` only for the requested source and new name/destination. Check installed `--help`; the [explicit symlink-object option](file-link-relocation.md) was merged in PR #64. Observe the source and the existing destination parent with `files info`, then supply their exact versions. One absolute root bounds both relative paths.

```bash
cueward files info --root /absolute/directory --path from/report.txt
cueward files info --root /absolute/directory --path to
cueward files move --root /absolute/directory --path from/report.txt --destination to/report.txt --expected-version '<source version>' --expected-parent-version '<to version>' --dry-run
cueward files move --root /absolute/directory --path from/report.txt --destination to/report.txt --expected-version '<source version>' --expected-parent-version '<to version>'
cueward files relocation receipt --operation-id '<returned or announced ID>'
```

Rename keeps the existing parent and takes --name; move takes an exact new relative --destination. Parents must exist and fresh source/parent versions are required. Available files/directories/packages and opaque aliases are supported; links require --link-itself and targets are never followed. Same-volume operations retain inode/reference text. Different-volume moves verify an independent destination first, then retain the original privately without permanent deletion. Receipt fields copy_operation_id, copied, retained_source_path, source_quarantined and source_path_absent describe that method; renamed=false is expected. Read the child with files receipt. The bound is 256 MiB/256 nodes/depth 32. A directory cannot move into its subtree. Conflicts, ancestor links and unknown availability are refused.

`--dry-run` returns rename_plan/move_plan observations including conflicts, same_filesystem and no_op. It makes no receipt, grants no execution token and does not establish native capability or reserve names. Execution requires supported guarded exclusive rename on the volume. Receipt storage need not share that volume.

The source version is checked before submission, not atomically bound to a kernel inode. External source replacement can be moved instead, including a replacement leaf symlink itself without following it. Detected identity/path differences produce incomplete evidence without rollback. Never describe this as locked paths or guaranteed selected-inode delivery. A success checks metadata/path observations, not payload hashes, recursive contents, all xattrs/ACLs or cloud sync.

Mutation JSON operations rename/move carry receipts. Exit zero requires status=completed and completion_verified=true; outer Ok alone does not suffice. Relocation_receipt lookup only reads saved evidence and can exit zero for a failed operation. Stages and null fields may predate interruption. Renamed=true means the native call succeeded, not that the requested source was verified. False means no rename by this operation, including a no-op; null is unknown.

- not_started: preflight or known native rejection, without a rename by this operation. External actors may have changed paths.
- incomplete: native success followed by verification/checkpoint failure; preserve all objects.
- uncertain: delivery/completion unknown, including timeout/lost response; actual names may already have changed.

Save the stderr operation_id before cancelling. Use `files relocation receipt --operation-id`, inspect before/actual post-observations and errors, then reobserve actual paths before deciding next action. Proposed destination_path is not an actual-location guarantee. Never automatically retry, roll back, delete, overwrite or replay the same ID. Parent SIGINT/SIGTERM/SIGKILL and timeout stop the worker but do not undo submitted rename. An interrupted parent cannot finalize evidence. Receipts remain in private ~/.cueward/operations/files-relocation directories without expiry; no payload backup is made.

Worker timeout is 1..30000 ms/default 10000. Preserve retained originals, child copy evidence and receipts indefinitely unless cleanup is separately authorized. Provider synchronization, actual TCC/read-only/unmount and older macOS remain unverified. For creation/copy/duplicate use [file mutations](file-mutations.md); trash/restore have separate commands and explicit removal confirmation. Never substitute permanent deletion.