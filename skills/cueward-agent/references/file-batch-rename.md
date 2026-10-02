# Batch rename planning and authorized independent execution

Use installed `files rename-batch plan` to inspect the user's explicitly selected proposals. Check installed `--help` before using these commands. Planning never renames files and does not authorize execution.

Observe each source and its existing parent with `files info`. Repeat `--entry` with a serializer-produced JSON object containing exactly path, name, expected_version and expected_parent_version:

```bash
cueward files rename-batch plan --root /absolute/directory \
  --entry '{"path":"report.txt","name":"report final.txt","expected_version":"<source version>","expected_parent_version":"<root version>"}' \
  --entry '{"path":"notes.txt","name":"notes final.txt","expected_version":"<source version>","expected_parent_version":"<root version>"}'
```

Each relative path keeps its existing parent and name is one exact new leaf. Do not concatenate unescaped external filenames into JSON. Planning has no mutation/receipt/dry-run flag. No glob/template/counter/suffix inference, manifest-file read, overwrite, parent creation or automatic dependency ordering is available. Input is 1..64 entries and the whole serialized request must fit 16 KiB; not every 64-entry batch fits. Timeout-ms defaults to 10000, range 1..30000, for the whole worker, not each item.

Available files/directories/packages/hardlinks use [single-item relocation planning](file-relocation.md). Directory contents and payload hashes are not inspected. Initial links/traversal, aliases/unknown alias state, placeholders/unknown availability and special files are unsupported. The no-materialization policy does not prove File Provider coordination or cloud sync. Planning requests no payload/selected metadata writes, Finder activation, downloads or mutation receipts; read-only transport may use private temporary files elsewhere.

JSON operation is rename_batch_plan. Exit zero / outer Ok only means observations returned, including conflicts or even all-item errors. Inspect items, every error, issues and has_conflicts. Each item preserves its zero-based input index and has either proposal or error, never an omitted input or invented empty source. Execution_supported is always false. Invalid outer JSON/count/budget/root, transport/deadline failure or changes during final rechecking return outer Err/nonzero.

Issues identify zero-based indices: existing_destination, different_filesystem, duplicate_source, shared_source_identity, duplicate_destination, potential_destination_collision, source_destination_dependency and nested_source. Exact same-path no-ops can be observed. Swaps/chains, shared hardlink revisions and ancestor-directory renames are not ordered or resolved. Conservative canonical-Unicode/case-insensitive comparison does not rewrite names and can flag false positives on case-sensitive volumes; no issues is not proof of collision freedom or native capability.

All items and rechecks use the initially observed canonical root, even when the supplied root is a supported directory symlink. The top-level request retains the supplied spelling; each proposal's request uses the canonical root path. Successful proposal roots must match the batch root's path, identity and version. Retargeting a root symlink cannot select another root for later items.

Successful initial observations are rechecked after the scan, and the supplied root mapping is checked again before return; detected revision/conflict changes discard the aggregate. Initial errors are not retried. This is sequential observation, not an atomic namespace snapshot, path lock, reservation, authorization or reusable token. Reobserve after changes and present the requested plan; do not loop over single-item rename using these stored versions because each rename changes revisions and partial completion/dependencies need separate handling.

## Authorized execution

Only after the user authorizes the explicit root, sources and new names, use `files rename-batch execute --root ... --entry '<same four-field JSON>'`. This source capability is pending review; check installed help. Do not pass a saved plan as an execution token. Reobserve if anything changed since planning. The same entry/count/byte bounds and one whole-worker deadline apply.

The worker preflights the entire batch before selected-path writes, then replans all remaining items before each ordered rename. Any item error or conflict/dependency refuses further execution, including swaps/chains, selected shared hardlinks, nested sources and conservative Unicode/case collisions. Source revisions remain original; parent/root revisions advance only from verified earlier child observations. Root requests stay bound to the initial canonical root and held descriptor, with the supplied alias mapping checked. Final destination/source-absence and parent/root checks are required for aggregate completion. No overwrite, automatic ordering, resume, retry or rollback is offered.

Execution JSON operation is rename_batch. Exit zero requires status=completed and completion_verified=true. Outer Ok alone is insufficient. Inspect every indexed item's child ID/status, mutation_attempted, renamed, completion_verified and error plus aggregate issues/error. Null child ID means not allocated; null renamed means unknown, not false. Aggregate issues keep the first 128 diagnostics and `omitted_issue_count` gives the exact number omitted (older receipts default to zero); nonzero means the issue array is incomplete. No input item is dropped and all conflicts still refuse further writes. A fresh plan describes current observations, not the full history of a failed receipt. Partial progress is incomplete; unknown submission or transport/interruption is uncertain. Individual completion does not override a later aggregate failure.

The aggregate ID/lookup is announced on stderr before dispatch. Use `files rename-batch receipt --operation-id '<aggregate ID>'` and `files relocation receipt --operation-id '<item child ID>'` to read persistent evidence, never resume. Child receipts contain actual before/after paths and identities and effective guards; the top-level request retains the user's original input. An active_index or started flag is only saved progress, not proof a process is still running. Lookup success only means evidence loaded. Interrupted aggregate checkpoints may lag child evidence. Do not retry pending items, roll back completed items or delete results automatically.

One parent lifeline/deadline covers all children. Cancellation can leave an earlier rename completed or submitted; termination never reverses it. Checks remain sequential, not source-inode CAS, isolation or provider sync proof. External directory edits during a native operation may be folded into its post-observed revision. Reobserve and ask for any new authorization needed to recover. Permanent deletion is never a fallback.

Dependency-aware execution and resume/rollback are not implemented. Real provider/TCC/unmount/read-only/external-volume and older macOS acceptance are unverified. Never substitute permanent deletion or an automatic overwrite/retry for an unsupported operation.
