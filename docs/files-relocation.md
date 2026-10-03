# Rename and same/cross-volume move

`files rename`, `files move`, their `--dry-run` plans and `files relocation receipt` implement operations of [issue #42](https://github.com/Termdock-dev/cueward/issues/42). Check the installed CLI's help before using it. The [explicit symlink-object option](files-link-relocation.md) was merged in PR #64. Use only the user's authorized source, new name/destination and absolute root.

## Select paths and observe versions

```bash
cueward files info --root /absolute/directory --path from/report.txt
cueward files info --root /absolute/directory --path from
cueward files rename --root /absolute/directory --path from/report.txt --name renamed.txt --expected-version '<source version>' --expected-parent-version '<from version>' --dry-run
cueward files rename --root /absolute/directory --path from/report.txt --name renamed.txt --expected-version '<source version>' --expected-parent-version '<from version>'
cueward files info --root /absolute/directory --path from/renamed.txt
cueward files info --root /absolute/directory --path to
cueward files move --root /absolute/directory --path from/renamed.txt --destination to/report.txt --expected-version '<fresh source version>' --expected-parent-version '<to version>'
cueward files relocation receipt --operation-id '<returned or announced ID>'
```

Both revisions are required. Rename takes one exact filename, preserves the existing parent and rejects empty names, separators, `.` and `..`. Move takes a new relative path under the same root; its parent must already exist. Neither creates missing parents, overwrites, merges, chooses another name, downloads data, requests Finder activation or permanently deletes an original. Reobserve after a mutation because source/parent versions can change.

Sources may be available regular files, directories, packages or hardlinked entries. Moving a hardlink moves that directory entry, not its other links. Packages move as directories without inspecting their contents or repairing internal references. Directory moves into themselves/descendants are rejected using ancestor identities. Initial symlinks are rejected unless [explicitly selected as link objects](files-link-relocation.md) with `--link-itself`; on the same volume this retains their exact inode/reference text without resolving or repairing targets. Finder aliases are relocated as opaque file objects. Traversal through ancestor links, unknown alias state, placeholders/unknown availability and special files remain rejected. Selected paths can lie inside packages; this is not bundle-aware management. An exact same-path request is a verified no-op without a native call. Case-only renames on case-insensitive volumes are rejected as conflicts when the destination already resolves; no intermediate-name workaround is used.

Dry runs return `rename_plan` or `move_plan`, with source/root/parent/resource observations, destination path/conflict, `same_filesystem` and `no_op`. They do not allocate receipts, claim success or grant an execution token. Existing destinations and different-device proposals can appear in plans without authorizing execution. A plan does not establish kernel/filesystem capability or lock anything.

## Protection and concurrency boundary

Same-volume execution uses one `renameatx_np` call with `RENAME_EXCL | RENAME_NOFOLLOW_ANY | RENAME_RESOLVE_BENEATH`, resolving both full relative paths from the held root descriptor. Existing destinations are never overwritten; unsupported guarded/exclusive support or a native cross-device error fails without a fallback. The adapter requires the destination volume's declared exclusive-renaming support and probes BENEATH rejection using only a newly owned private empty file. Receipt storage need not share the source/destination volume because payload is not staged there.

Expected versions are preflight checks, not an atomic source-inode condition. Another process can replace the source after the last check, and the kernel can move that replacement. In particular, a replacement leaf symlink can be moved as a link without following its target. Post-checks detect observed identity/path differences and retain an incomplete receipt; they cannot undo the operation or locate every externally relocated object. The held root anchors a directory object, not a permanently fixed absolute path. This is not isolation against malicious namespace swaps.

The operation does not rewrite payload, xattrs, permissions or ACLs. Successful completion checks destination identity/type/availability/size/mtime/mode/birthtime against the selected source, its current revision against the held source descriptor, source-path absence (except a no-op), and root/parent descriptor/path identities and post-observation revisions. Rename may change ctime, so the original full source version need not survive. These are sequential metadata checks, not payload hashes, recursive tree verification, ACL/xattr verification or a snapshot of concurrent content edits. Native rename retains the object and its metadata, without destination ACL inheritance adjustment or cloud coordination.

## Different-volume moves

When source and destination parents are on different devices, move reuses the verified copier rather than native cross-device rename. It first creates and verifies an independent destination with no overwrite, then rechecks source/destination and moves the original into private same-source-volume storage. The original is retained, never permanently unlinked. Directory/package contents, aliases and selected `--link-itself` references use the same bounded copier. The fixed copy bound is 256 MiB, 256 nodes and depth 32; the ordinary relocation deadline remains in force.

The relocation receipt records `copy_operation_id` (read with `files receipt`), `copied`, `retained_source_path`, `source_quarantined`, destination identity and source-path absence. `renamed=false` is expected for this copy/retain method; it does not mean no mutation. Successful completion requires both verified destination and retained original. Failure after copy can leave both source and destination; after quarantine the original can be at the recorded private path. Neither receipt is an automatic replay/recovery token. Retention can consume space indefinitely, including a `.cueward-stage-<copy-id>` directory on an external source volume.

An owned APFS-image release probe on macOS 27.0.1 (2026-10-03) verified file/directory/link moves in both directions, independent destination bytes/metadata and retained original readability. This does not establish arbitrary provider, physical external-drive, read-only/unmount, cancellation-at-every-cross-volume-stage or older-macOS behavior.

## Receipts, interruption and failures

External JSON operations are `rename`, `move`, `rename_plan`, `move_plan` or `relocation_receipt`. Mutation commands exit zero only for `status=completed` and `completion_verified=true`; outer `Ok` alone is insufficient. Receipt lookup exits zero when evidence loads, even if the recorded operation failed. Dry-run success means observations were returned, including possible conflicts.

Receipts contain operation ID/path, the original request, `before` plan, stages `preflight/prepared/renaming/verifying/finished`, `mutation_attempted`, optional `renamed`, completion verification, error, and actual post-call root/source/parent/destination FileInfo observations. `source_path_absent=null` means unobserved, not absent. `destination_after.path` is the observed result, while `before.destination_path` is only a proposed path. No automatic search, recovery or reversal occurs.

| Status | Meaning |
| --- | --- |
| completed | Requested relocation or exact no-op passed the post-checks and final checkpoint. |
| not_started | Preflight or a known native rejection prevented this operation from renaming. External actors may still have changed paths. |
| incomplete | Native rename succeeded, then verification/checkpoint failed. Preserve the moved object and other observed objects. |
| uncertain | Submission/completion cannot be established, including timeout or lost response. Names may already have changed. |

`renamed=true` means native success, not verified completion or proof that the selected source moved. False indicates no rename by this operation, including no-ops; null is unknown. Stages/flags can predate interruption and are not proof of current state. Inspect the error and actual paths, then reobserve before any further action. Do not automatically retry, overwrite, delete, roll back or reuse the same operation ID.

Mode-0700 directories under `~/.cueward/operations/files-relocation/<id>/` contain mode-0600 receipts/claims. Same-volume relocation needs no payload backup; different-volume moves retain the original as described below. Receipts persist without automatic expiry; checkpoints are synced and atomically replaced, not a power-loss transaction. The parent flushes operation ID and lookup command on stderr before dispatch and compares the response with stored evidence. The worker claims each ID once.

A parent-owned socket lifeline stops the worker on EOF/error/unexpected further input, including parent SIGINT/SIGTERM/SIGKILL, and rejects dead-parent delayed startup. Timeout stops the owned group and returns uncertain evidence. Neither cancellation nor process termination reverses an already submitted rename; an interrupted parent cannot finalize its response, so use the announced ID to inspect the last checkpoint. `--timeout-ms` is 1..30000, default 10000, covering the worker; parent receipt preparation/finalization is outside that deadline.

No-materialization policy is enabled before filesystem execution, but `provider_coordination=filesystem_only_provider_state_unknown`. Provider synchronization, server-side completion and other processes remain outside this contract. Errors are never verified empty results.

## Acceptance and remaining work

Disposable macOS tests cover native file/directory/package/hardlink relocation, exact Unicode/newline names, same-name no-op, inode/bytes/mode/mtime/birthtime/xattr retention, stale guards, directory descendants, existing and late file/directory/broken-link conflicts, late source replacement (including a leaf link), destination-parent relocation/link replacement, post-call parent replacement, checkpoint failures, permission denial, different-device observations, saved receipts/replay rejection, timeout and delayed/armed parent interruption. A release CLI probe passed on macOS 27.0.1 on 2026-10-02 for dry-run, same-inode/same-bytes move, rename/no-op, no-overwrite conflict, escaped output and saved receipts. Real external-volume, read-only/unmounted volume, File Provider/TCC and older macOS acceptance remain unverified.

Copy/duplicate support files, directory/package trees, opaque aliases and link objects through [file mutations](files-mutations.md). Confirmed [trash](files-trash-execution.md), [backup restoration](files-trash-restore.md), [existing tags](files-tags.md) and [independent batch rename](files-batch-execution.md) have their own commands. Permanent deletion, automatic swaps/chains and provider synchronization are excluded.