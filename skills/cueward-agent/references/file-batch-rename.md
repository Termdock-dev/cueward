# Read-only batch rename planning

Use installed `files rename-batch plan` to inspect the user's explicitly selected proposals. This source capability is pending review; check `--help`. Planning never renames files and does not authorize execution.

Observe each source and its existing parent with `files info`. Repeat `--entry` with a serializer-produced JSON object containing exactly path, name, expected_version and expected_parent_version:

```bash
cueward files rename-batch plan --root /absolute/directory \
  --entry '{"path":"report.txt","name":"report final.txt","expected_version":"<source version>","expected_parent_version":"<root version>"}' \
  --entry '{"path":"notes.txt","name":"notes final.txt","expected_version":"<source version>","expected_parent_version":"<root version>"}'
```

Each relative path keeps its existing parent and name is one exact new leaf. Do not concatenate unescaped external filenames into JSON. No glob/template/counter/suffix inference, manifest-file read, overwrite, parent creation, automatic ordering, execute, receipt or dry-run option is available. Input is 1..64 entries and the whole serialized request must fit 16 KiB; not every 64-entry batch fits. Timeout-ms defaults to 10000, range 1..30000, for the whole worker, not each item.

Available files/directories/packages/hardlinks use [single-item relocation planning](file-relocation.md). Directory contents and payload hashes are not inspected. Initial links/traversal, aliases/unknown alias state, placeholders/unknown availability and special files are unsupported. The no-materialization policy does not prove File Provider coordination or cloud sync. Planning requests no payload/selected metadata writes, Finder activation, downloads or mutation receipts; read-only transport may use private temporary files elsewhere.

JSON operation is rename_batch_plan. Exit zero / outer Ok only means observations returned, including conflicts or even all-item errors. Inspect items, every error, issues and has_conflicts. Each item preserves its zero-based input index and has either proposal or error, never an omitted input or invented empty source. Execution_supported is always false. Invalid outer JSON/count/budget/root, transport/deadline failure or changes during final rechecking return outer Err/nonzero.

Issues identify zero-based indices: existing_destination, different_filesystem, duplicate_source, shared_source_identity, duplicate_destination, potential_destination_collision, source_destination_dependency and nested_source. Exact same-path no-ops can be observed. Swaps/chains, shared hardlink revisions and ancestor-directory renames are not ordered or resolved. Conservative canonical-Unicode/case-insensitive comparison does not rewrite names and can flag false positives on case-sensitive volumes; no issues is not proof of collision freedom or native capability.

Successful initial observations are rechecked after the scan; detected revision/conflict changes discard the aggregate. Initial errors are not retried. This is sequential observation, not an atomic namespace snapshot, path lock, reservation, authorization or reusable token. Reobserve after changes and present the requested plan; do not loop over single-item rename using these stored versions because each rename changes revisions and partial completion/dependencies need separate handling.

Batch execution and its persistent partial-failure evidence remain future #42 work. Real provider/TCC/unmount/read-only/external-volume and older macOS acceptance are unverified. Never substitute permanent deletion or an automatic overwrite/retry for an unsupported operation.
