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

Mkdir creates one empty directory (mode 0700 subject to umask), without creating parents. Copy creates one independent regular file within the same root. Neither operation overwrites, merges, auto-renames conflicts or follows links. Copy rejects source directories/packages, Finder aliases, unknown alias state, placeholders/unknown availability, special permission bits and compressed sources. Existing destination parents are scoped directories without package-ancestry pruning; explicit paths can select package internals. Hardlinked source content can be copied without changing its links. Move/rename/recursive copy/duplicate/trash/tag edits are not available in this slice. Never substitute permanent deletion for trash.

Copy max-bytes defaults to 67108864, range 1..268435456. Native xattrs have separate fixed 65536-byte name/4194304-byte aggregate value limits. Timeout-ms defaults to 10000, range 1..30000, covering the worker's preparation/write/verification/checkpoints; parent receipt preparation/finalization is outside that deadline.

The external JSON operation is mkdir/copy/receipt. Outer Ok can carry a failed/uncertain mutation. Mkdir/copy exit nonzero unless status=completed and completion_verified=true. Receipt exits zero when saved evidence loads, even for unsuccessful writes. Interpret status:

- completed: requested object and postconditions verified at final checkpoint, not a guarantee of cloud sync or unchanged future state.
- not_started: known preflight/creation rejection, with no requested destination creation.
- incomplete: known creation followed by failure; partial or complete-looking destination remains.
- uncertain: delivery/completion cannot be established; a destination may exist even if the last checkpoint records mutation_attempted=false.

Keep operation_id, receipt_path, stage/error, destination_created and before/post/initial-created observations. Saved receipts are checkpoints, not current filesystem state. Copy verification reports data bytes/SHA-256/read-back readability, permissions/mtime equality and matching bounded xattrs. Owner/group/ACL/birth time/access time/file flags/sparse layout/hardlink relationships are not verified preservation. Source bytes are never edited or removed.

On incomplete/uncertain, read the saved receipt, reobserve actual paths and inspect any destination before deciding what to do. Do not automatically retry, delete a partial destination or expand the scope. The same operation ID cannot be replayed; files receipt does not resume or reconcile. Parent and worker keep private 0700 operation directories with 0600 receipts at ~/.cueward/operations/files/<id>/receipt.json. Evidence has no automatic expiry; keep it until any uncertain outcome is resolved. A stopped/crashed worker can leave an earlier uncertain checkpoint.

Provider_coordination=filesystem_only_provider_state_unknown. Dataless materialization is denied and no download API is called, but this is not generic File Provider coordination or an atomic filesystem transaction. Descriptor/version checks detect observed changes, not malicious concurrent swaps or all server-side activity after process termination. Cross-volume, true TCC/unmount/read-only-volume/provider and older macOS behavior remain unverified; do not infer those from local fixture success.
