# Authorized no-overwrite file creation and copy

Use installed `files mkdir/copy/receipt` only for the user's requested effect and selected source/destination. Check each command's --help: these source instructions can be newer than main or a release. A prior FileInfo version is not permission to write, overwrite, delete or retry.

Observe the source and destination's existing parent with `files info`. Both commands require one absolute root and expected-parent-version; copy also requires the source expected-version. Reobserve parents after creation because their versions change.

```bash
cueward files info --root /absolute/directory --path .
cueward files mkdir --root /absolute/directory --path Reports --expected-parent-version '<observed parent version>'
cueward files info --root /absolute/directory --path report.txt
cueward files info --root /absolute/directory --path Reports
cueward files copy --root /absolute/directory --path report.txt --destination Reports/report.txt --expected-version '<observed source version>' --expected-parent-version '<observed Reports version>'
cueward files receipt --operation-id '<returned operation ID>'
```

Mkdir creates one empty directory (mode 0700 subject to umask), without creating parents. Copy creates one independent regular file within the same root. Neither operation overwrites, merges, auto-renames conflicts or follows links. Copy rejects source directories/packages, Finder aliases, unknown alias state, placeholders/unknown availability, special permission bits and compressed sources. Existing destination parents are scoped directories without package-ancestry pruning; explicit paths can select package internals. Hardlinked source content can be copied without changing its links. Tag observations and authorized initial tagging (not existing-tag edits) use the separate [file-tags reference](file-tags.md). Move/rename/recursive copy/duplicate/trash are not available in this slice. Never substitute permanent deletion for trash.

Copy max-bytes defaults to 67108864, range 1..268435456. Native xattrs have separate fixed 65536-byte name/4194304-byte aggregate value limits. Timeout-ms defaults to 10000, range 1..30000, covering the worker's preparation/write/verification/checkpoints; parent receipt preparation/finalization is outside that deadline.

Objects are fully prepared/verified in the private receipt operation directory, then atomically published using exclusive, no-follow, beneath-root rename from the held root descriptor. The worker performs no payload/metadata writes after publication. A cached parent moved outside root is never used for delivery. This does not prevent another actor from moving an already published object; the anchor identifies the selected directory object, not a permanently fixed absolute path.

The selected root must share a filesystem with ~/.cueward/operations/files. Different-device roots and unsupported kernel/filesystem guards return unavailable; do not work around this refusal with an unguarded copy. No fallback is offered. ACL inheritance comes from private staging, not the destination parent, and ACL preservation is not guaranteed.

Stages staging/copying/staged describe private preparation, creating describes target publication. Mutation_attempted concerns target publication, not staging writes. Staging_verified is not proof of delivery. Staging_path normally disappears on success but can retain empty, partial or fully copied source content after failure/cancellation, bounded by the copy budgets. Old receipts without these fields remain readable.

The external JSON operation is mkdir/copy/receipt. Outer Ok can carry a failed/uncertain mutation. Mkdir/copy exit nonzero unless status=completed and completion_verified=true. Receipt exits zero when saved evidence loads, even for unsuccessful writes. Interpret status:

- completed: requested object and postconditions verified at final checkpoint, not a guarantee of cloud sync or unchanged future state.
- not_started: known preflight/staging/publication rejection, with no requested destination creation; private staged content may remain.
- incomplete: known creation followed by failure; partial or complete-looking destination remains.
- uncertain: delivery/completion cannot be established; a destination may exist even if the last checkpoint records mutation_attempted=false.

Keep operation_id, receipt_path, staging_path/staging_verified, stage/error, destination_created and before/post/initial-created observations. Saved receipts are checkpoints, not current filesystem state. Copy verification reports data bytes/SHA-256/read-back readability, permissions/mtime equality and matching bounded xattrs. Owner/group/ACL/birth time/access time/file flags/sparse layout/hardlink relationships are not verified preservation. Source bytes are never edited or removed. After final read-back, completion rechecks the parent and destination paths against their descriptors and earlier revisions. Detected relocation/replacement leaves an incomplete outcome and preserves written/replacement objects; these checks do not lock paths against subsequent changes.

On any failed/uncertain operation, read the saved receipt, reobserve actual paths and inspect both any destination and private staging_path before deciding what to do. Do not automatically retry, delete a partial destination or expand the scope. The same operation ID cannot be replayed; files receipt does not resume or reconcile. Parent and worker keep private 0700 operation directories with 0600 receipts at ~/.cueward/operations/files/<id>/receipt.json. Private staged content can remain alongside evidence indefinitely, including after not_started; there is no automatic cleanup or expiry. Do not delete it without the user’s cleanup authorization.

Before dispatch, mkdir/copy flush a generated operation_id and a receipt lookup command to stderr without changing final stdout JSON. Preserve this ID, including when cancelling the CLI. Parent SIGINT/SIGTERM/SIGKILL closes the worker lifeline and stops the writer; a parent already dead at startup prevents dispatch into the mutation. This does not undo private staging or submitted publication/provider work. An interrupted parent cannot finalize its receipt or return a final response: read the last checkpoint using the announced ID and inspect actual paths. Earlier uncertain or already-verified completed evidence may remain; do not infer absence from cancellation.

Provider_coordination=filesystem_only_provider_state_unknown. Dataless materialization is denied and no download API is called, but this is not generic File Provider coordination or an atomic filesystem transaction. Descriptor/version checks detect observed changes, not malicious concurrent swaps or all server-side activity after process termination. Cross-volume publication is unsupported. True TCC/unmount/read-only-volume/provider and older macOS behavior remain unverified; do not infer those from local fixture success.
