# Independent batch rename execution

`files rename-batch execute` continues [issue #42](https://github.com/Termdock-dev/cueward/issues/42) with fail-stop execution of explicitly selected independent sibling renames. This source capability is pending review; check installed command help. [Planning](files-batch-rename.md) remains read-only and never authorizes execution.

## Request and authorization

First observe each source and its parent with `files info`, then inspect the explicit `files rename-batch plan` result. Execute only the user's authorized root, paths and exact new names:

```bash
cueward files rename-batch execute --root /absolute/directory \
  --entry '{"path":"report.txt","name":"report final.txt","expected_version":"<report version>","expected_parent_version":"<root version>"}' \
  --entry '{"path":"notes.txt","name":"notes final.txt","expected_version":"<notes version>","expected_parent_version":"<root version>"}'
cueward files rename-batch receipt --operation-id '<announced aggregate ID>'
cueward files relocation receipt --operation-id '<item child ID>'
```

Use a JSON serializer for external filenames. Input order is execution order. The same 1..64-entry and complete serialized-request 16 KiB limits apply; not every 64-entry batch fits. One worker deadline covers the entire batch, not each item: `--timeout-ms` defaults to 10000, range 1..30000. Parent receipt preparation/finalization is outside the worker deadline.

The entire batch is preflighted before any selected-path mutation. Any item error or issue rejects execution, including stale/missing/unavailable sources, existing targets, duplicate sources/destinations, selected hardlinks sharing an identity, conservative case/Unicode collisions, swaps/chains, nested sources and different-device observations. There is no dependency ordering, intermediate filename, overwrite, suffix generation, manifest-file read, resume, retry or rollback option. Same-path no-ops may complete without a native call.

Sources and platform restrictions are inherited from [single-item relocation](files-relocation.md): available regular files/directories/packages and independently selected hardlinked entries, no initial source links/link traversal, Finder aliases/unknown alias state, placeholders/unknown availability or special files. There is no payload copy, permanent deletion, recursive inspection, download or Finder activation.

## Revision advancement and root boundary

The batch holds its initial canonical root descriptor throughout execution. Item requests use that canonical root; the aggregate request preserves the original supplied spelling. The supplied root mapping, held root identity/version and canonical path observation are checked before every item. The child's held preflight root must also match the expected batch root before native submission.

Before every child, all remaining proposals are replanned. Their source versions remain the user's original guards. Parent revisions advance only for the same observed parent path/identity, using verified post-observations of earlier successful children. The expected root revision similarly advances from the verified child result. Thus two sibling renames can proceed without reusing a stale parent token or blindly accepting an arbitrary fresh parent revision. Changes to another parent retain its original guard. An item/issue failure in the remaining batch stops all further submissions, not just that item.

Each child uses the existing guarded exclusive native rename and single-item post-checks. The aggregate requires a final recheck of every completed destination revision, original source-path absence except no-ops, latest parent revisions and root descriptor/path/mapping before declaring completion.

These are sequential observations, not atomic source-inode CAS, a namespace snapshot, reservation or malicious-swap isolation. External changes during a native operation can be included in its post-observed directory revision; advancing that revision does not prove only this batch changed the directory. A source replacement after the last check can be renamed by the kernel; post-checks can detect observed mismatches but cannot undo the operation. Changes after final checks remain possible. Contents, recursive trees, ACLs/xattrs and provider sync are not independently verified. A clear read-only plan remains `execution_supported=false`: that plan is not a reusable execution token, even when the separate execute command is installed.

## Aggregate and child receipts

External operations are `rename_batch` and `rename_batch_receipt`. Execute exits zero only when aggregate `status=completed` and `completion_verified=true`; outer Ok alone is insufficient. Lookup exits zero when saved evidence loads, even for a failed or interrupted batch, and never resumes execution.

The aggregate records its original request, initial root observation, `started`, `active_index`, indexed `items`, preflight/remaining `issues`, status, completion verification and error.

Aggregate issues retain at most the first 128 diagnostics in collection order; `omitted_issue_count` reports the exact number of additional diagnostics, not missing input items. A nonzero omission count means the saved issues array is incomplete, never conflict-free. Small issue lists are unchanged. Older receipts without this field decode as zero omissions because they kept their full lists. All detected conflicts still refuse further child submissions; compaction changes stored diagnostics only. Use a fresh read-only plan to inspect current conflicts, not to reconstruct interrupted history or authorize replay.

Each item retains its original zero-based index, nullable child `operation_id`/`receipt_path`/status, mutation-attempt flag, nullable `renamed`, completion verification and error. Remaining-plan issue indices map back to original input indices. A null child ID means no child receipt was allocated, not proof an external actor left that path unchanged. `active_index` is a saved checkpoint, not a live process indicator.

Allocated children have separate full relocation receipts with effective source/parent guards, before observations, native stages and actual post-call paths/identities. Consult them even when the aggregate failed or its flags are older than the child checkpoint. Per-item completion records what passed that child's checks at that time; it does not override a later aggregate final-check failure.

| Aggregate status | Meaning |
| --- | --- |
| completed | All requested renames/no-ops and final aggregate checks passed and were checkpointed. |
| not_started | No child completed or is known to have renamed, and no unknown submission is recorded. Known native rejection can have mutation_attempted=true. |
| incomplete | Some child completed/renamed, or final checks/checkpointing failed after progress. Remaining entries were not automatically submitted. |
| uncertain | Native submission may have happened, or transport/deadline/lifetime interruption prevented authoritative final evidence. |

A worker crash or parent interruption can leave the initial/last aggregate status uncertain even when a child receipt later says completed. Never reconstruct permission to continue from these flags. Reobserve actual paths and obtain new authorization as needed; do not automatically replay pending entries, overwrite a target, delete a moved object or roll back verified children.

Mode-0700 aggregate directories under `~/.cueward/operations/files-batch-rename/<id>/` hold mode-0600 bounded evidence and one-use claims. Child evidence uses the existing files-relocation namespace. Both persist without automatic expiry. The aggregate ID and lookup command are flushed on stderr before dispatch. Child and aggregate checkpoints precede native submission; storage checkpoints are not an atomic cross-receipt or power-loss transaction. An allocated child is referenced in the aggregate before it can mutate selected paths. The parent compares the worker response to prepared and saved evidence.

One parent-owned socket lifeline covers the entire worker, including every child. Parent EOF/error or SIGINT/SIGTERM/SIGKILL stops it, even after earlier native progress. Timeout stops the owned process group and returns uncertain evidence. Termination never reverses submitted native operations. The no-materialization policy is enabled, but provider coordination remains filesystem-only with unknown provider state.

## Acceptance and remaining work

Owned disposable tests cover sibling parent-revision advancement, original root-alias preservation, separate parents, packages, no-ops, full-batch conflict/item-error refusal, dense conflicts at native 50-entry and short-opaque-revision 64-entry request bounds with saved not-started evidence, stale pending sources, late targets/parent changes, root-alias retargeting, child root-anchor mismatch, known native rejection/unknown submission, checkpoint failures, final destination changes, saved aggregate/child lookup, replay/shape refusal, escaped JSON and timeout. Delayed and armed workers are stopped by parent signals, including an armed worker after its first actual native rename; later items remain untouched and partial evidence survives.

Real File Provider/TCC, external/read-only/unmounted volumes and older macOS acceptance remain unverified. Dependency-aware swaps/chains, execution resume/rollback, package duplication, broader trash types/restore, cross-volume operations and safe existing-tag edits remain separate work. Ordinary tree copies use the separate [execution contract](files-copy-tree-execution.md) merged in PR #59; check installed help. Confirmed ordinary-file trash uses [verified trash execution](files-trash-execution.md). Permanent deletion is excluded.
