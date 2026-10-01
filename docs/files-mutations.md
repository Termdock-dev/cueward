# No-overwrite directory creation and single-file copy

`cueward files mkdir`, `files copy` and `files receipt` are the first slice of [issue #42](https://github.com/Termdock-dev/cueward/issues/42). Check installed command help before relying on newer source capabilities. Use these commands only for the user's requested creation/copy and selected source/destination; a metadata version is an observation, not authorization.

## Observe, then select the destination

Both mutations require one absolute `--root`, a new relative destination and `--expected-parent-version` from `files info` on the destination's existing parent. Copy additionally requires `--expected-version` from the source's FileInfo. Creating a directory changes its parent's revision, so reobserve before another write. Neither command creates missing parents, follows symlinks, overwrites, merges or auto-renames conflicts.

```bash
cueward files info --root /Users/me/Documents --path .
cueward files mkdir --root /Users/me/Documents --path Reports --expected-parent-version '<observed parent version>'
cueward files info --root /Users/me/Documents --path report.txt
cueward files info --root /Users/me/Documents --path Reports
cueward files copy --root /Users/me/Documents --path report.txt --destination Reports/report.txt --expected-version '<observed source version>' --expected-parent-version '<observed Reports version>'
cueward files receipt --operation-id '<returned operation ID>'
```

`mkdir --path` creates one empty directory with mode 0700, subject to the process umask. `copy --path` selects the source and `--destination` selects a new file within the same root. Copy supports one available regular file, including empty files and hardlinked sources; the destination is an independent file, not a hardlink/clone relationship. Copy rejects source directory trees/packages, Finder aliases, dataless/unknown source availability, special permission bits and compressed sources. Both commands reject traversal through symlinks. Alias state must be available/false. There is no link-following option in this slice. Destination parents are scoped filesystem directories, without package-ancestry pruning; an explicitly selected path can lie inside a package, but this does not provide bundle-aware management.

`--max-bytes` for copy defaults to 67,108,864 and accepts 1..268,435,456 data-fork bytes. The reported size and streamed growth are bounded. Native extended attribute names are limited to 65,536 bytes and aggregate values to 4,194,304 bytes; these are separate fixed metadata budgets. Oversized metadata is rejected during preflight and rechecked during copying. `--timeout-ms` is 1..30,000 (default 10,000), covering preparation, copying, metadata, read-back verification and checkpoint writes in the worker. Parent-side receipt preparation/finalization is outside the worker deadline. Accepted byte budgets do not bound native peak allocation or transient partial metadata under concurrent changes.

## Receipt and exit semantics

The external JSON envelope is `{"Ok":{"operation":"mkdir|copy|receipt","result":{...}}}`. Preparation/transport/receipt lookup can instead return structured `Err`. Names, paths and diagnostics are external data, escaped without altering decoded strings.

A loaded mutation receipt is not necessarily a successful write. `mkdir`/`copy` exit nonzero unless status is completed and completion_verified is true, even when the outer JSON is Ok. `receipt` exits zero when saved evidence was loaded, regardless of the saved mutation status. Always inspect:

| Status | Meaning |
| --- | --- |
| `completed` | The requested object and postconditions were verified before the final checkpoint. |
| `not_started` | A known preflight or creation rejection prevented the requested destination creation. An existing destination is untouched. |
| `incomplete` | Destination creation is known; a later error/verification/checkpoint failure occurred. A partial or complete-looking destination can remain. |
| `uncertain` | Delivery/creation/completion cannot be established, including a stopped worker or invalid/lost response. Destination can exist even when the last checkpoint says mutation_attempted=false. |

Receipts include operation_id, the original request, status/stage/error, mutation_attempted, optional destination_created, before/post FileInfo observations for root/parent/source/destination, the initial destination_created_observation, optional copy verification, completion_verified, receipt_path and provider_coordination. A before/post observation is not a transaction: null means no successful observation, and an earlier checkpoint can predate interruption. Failed/uncertain receipts must not be interpreted as current filesystem state or proof of absence. Provider coordination is `filesystem_only_provider_state_unknown`, not a cloud completion guarantee.

Copy verification includes SHA-256/bytes of streamed source content and independently read-back destination content, equal permissions/mtime, and a matching extended-attribute digest/aggregate value bytes. The final destination path is reopened and read again. Root/parent/source identities and path revisions are rechecked; the directory's own expected creation change is allowed. Source bytes are never edited, moved or removed.

No mutation is automatically retried or rolled back. Do not replay a failed/uncertain copy or delete its destination automatically: inspect its saved evidence and reobserve the actual paths, then obtain any additional user decision needed for cleanup or a new destination. The worker claims each operation once; a second dispatch of the same operation ID is rejected. `files receipt` only reads the saved checkpoint, without resuming, reconciling or re-executing it.

## Metadata and lifecycle

The macOS adapter uses guarded directory/source descriptors and descriptor-relative exclusive creation, rather than a path-based overwrite copy. The generic core streams and hashes bounded data; the adapter copies extended attributes with `fcopyfile(COPYFILE_XATTR)` and verifies all enumerated names/values against the source. Ordinary POSIX permission bits and modification time are set and compared. Owner/group, ACLs, birth time, access time, file flags, sparse layout, hardlink relationships and cloud/provider metadata coordination are not preserved/verified by this contract. Setuid/setgid/sticky sources and compressed sources are excluded rather than copied with uncertain security/compression semantics. Metadata precision/support differences can cause an incomplete verification result.

Receipts live in mode-0700 operation directories at `~/.cueward/operations/files/<operation-id>/receipt.json`; receipt/claim files are mode 0600. The parent prepares a receipt before dispatch. The worker syncs and atomically replaces checkpoints before creation and at later stages; the parent checks the returned receipt against stored evidence. Before dispatch, the parent flushes a generated `operation_id` and a `files receipt --operation-id` command to stderr; the final external JSON on stdout is unchanged. Save this progress line to retrieve evidence if the CLI is interrupted. Failure to publish it prevents worker dispatch.

Mutation workers use a parent-owned stdin socket for newline-framed input and lifetime monitoring. EOF, transport failure or unexpected subsequent input stops the entire worker; parent SIGINT, SIGTERM and SIGKILL close this lifeline even though the worker has its own process group. A dead parent before worker startup is rejected before mutation. No process-global signal handler or PID polling is used. A timeout still stops the owned process group and records uncertain. Parent interruption cannot return a final response or finalize the receipt, so use the announced ID to read the last saved checkpoint. Termination is not rollback: already submitted filesystem/provider writes can leave a partial or complete-looking destination. A previously verified final checkpoint can remain completed if the operation finished before interruption. There is no automatic evidence expiry, partial-file deletion or rollback. These checkpoints support process interruption, not a power-loss durability guarantee or an atomic multi-object filesystem transaction.

The filesystem worker denies dataless materialization and never calls a download API. It does not coordinate generic provider services or prove cloud synchronization. Paths/versions detect observed changes, not malicious concurrent swaps or every server-side operation after worker termination. Concurrent parent relocation can invalidate completion even if a descriptor-relative write already occurred. Network/provider/volume/TCC errors remain errors or uncertain outcomes, not verified empty content.

## Acceptance and remaining work

Disposable tests cover content/hash/readability, empty files, hardlinked sources, Unicode/newline/external-marker paths, permissions/mtime/native xattrs, existing/broken-link conflicts, late conflicts, stale source/parent revisions, path scope, source directories/packages/links/aliases, metadata budgets, permission denial, post-copy source change, checkpoint failure, stopped-worker uncertainty, delayed-start and armed-worker parent SIGINT/SIGTERM/SIGKILL interruption, pre-dispatch operation ID publication, private saved receipts and rejected replay. They do not mutate personal files.

Cross-volume copies, true read-only/unmounted volumes, real File Provider/TCC behavior, ACL inheritance and older macOS remain unverified. Move/rename/duplicate, recursive copy, trash/recovery, tag add/remove and batch plans remain subsequent #42 work. Permanent deletion is not included.

Native reference: Apple's [copyfile implementation/manpage](https://github.com/apple-oss-distributions/copyfile/blob/main/copyfile.3) distinguishes data, xattr and other metadata flags. The Darwin declarations/constants were checked against the local macOS SDK; this slice requests only COPYFILE_XATTR, not full Finder copy equivalence.
