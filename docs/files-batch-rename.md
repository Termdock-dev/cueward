# Read-only batch rename plans

`files rename-batch plan` adds a planning slice of [issue #42](https://github.com/Termdock-dev/cueward/issues/42). Check installed command help. New [per-entry link-object selection](files-batch-link-rename.md) was merged in PR #65. Planning does not rename anything; authorized independent batches use the separate [execution command](files-batch-execution.md).

## Explicit proposals

Observe each selected source and its existing parent using `files info`. Supply one absolute root and repeat `--entry` with a JSON object containing four required fields: relative `path`, exact new leaf `name`, source `expected_version` and parent `expected_parent_version`.

```bash
cueward files info --root /absolute/directory --path report.txt
cueward files info --root /absolute/directory --path .
cueward files info --root /absolute/directory --path notes.txt
cueward files rename-batch plan --root /absolute/directory \
  --entry '{"path":"report.txt","name":"report final.txt","expected_version":"<report version>","expected_parent_version":"<root version>"}' \
  --entry '{"path":"notes.txt","name":"notes final.txt","expected_version":"<notes version>","expected_parent_version":"<root version>"}'
```

Do not concatenate unescaped filenames into JSON; use a JSON serializer. Entries retain input order and decoded Unicode/newline names. No glob, template, counter, inferred suffix, manifest-file read, merge, overwrite, missing-parent creation or automatic ordering is provided. Each proposal keeps its existing parent. The command has no execute, receipt or dry-run flag: planning is its only mode.

There must be 1..64 entries and the entire serialized root/entries worker request must fit 16 KiB, including escaping and JSON structure. The count ceiling does not promise that all 64 entries fit. Unknown JSON fields, missing fields, wrong types or malformed entry JSON fail the outer request. `--timeout-ms` defaults to 10000, range 1..30000, for one worker's entire batch; it is not a per-item allowance. Parent-side argument parsing is outside this deadline.

Available regular files, directories, packages and hardlinked entries use the existing [relocation planning contract](files-relocation.md). Contents are not recursively inspected or hashed. Source links remain unsupported by default; the new optional per-entry boolean `link_itself=true` requires a leaf symlink and uses the [explicit link-object contract](files-batch-link-rename.md). Intermediate link traversal, Finder unknown alias state, placeholders/unknown availability and special files remain unsupported. Existing destination leaf links are observed as conflicts, not followed. No payload or selected metadata writes, Finder activation, provider downloads or mutation receipts are requested. Read-only process transport can use private temporary files outside the selected root.

## JSON and exit semantics

External JSON uses `{"Ok":{"operation":"rename_batch_plan","result":{...}}}` or structured `Err`. Names, paths and errors use the shared escaped external envelope.

The plan includes the original request, root observation, one `items` entry per input, indexed `issues`, `has_conflicts` and `execution_supported=false`. Each item has a zero-based `index` and either a full single-item `proposal` or an `error`; the other field is null. Invalid names, stale versions, unavailable sources and missing paths remain explicit item errors, never empty/missing sources or silently dropped inputs.

Exit zero / outer Ok means the observation result was returned, including possible conflicts or even an error for every item. It does not mean any mutation completed or the batch is executable. Always inspect every item/error, issue and `has_conflicts`. Root/scope failure, invalid outer input, transport/deadline failure or a detected change during final rechecking returns outer Err and nonzero instead of a partially validated plan.

## Conflicts and dependencies

`has_conflicts` is true for any item error or issue. Issue `entries` contains one or two zero-based item indices:

| Code | Observation |
| --- | --- |
| existing_destination | A non-no-op destination already exists, including a broken link or another source. |
| different_filesystem | Source and destination parent do not share the observed filesystem. No copy fallback is offered. |
| duplicate_source | The same observed source path was selected more than once. |
| shared_source_identity | Different observed paths share an inode identity, including hardlinks or equivalent path spellings; their revision guards are not independent. |
| duplicate_destination | Proposed leaves match exactly under the same observed parent identity. |
| potential_destination_collision | Different leaves compare equal under conservative case-insensitive/canonical-Unicode comparison in the same parent. |
| source_destination_dependency | A proposed destination can collide with another selected source; chains/swaps are not reordered or resolved. |
| nested_source | A selected directory can contain another selected source parent; renaming it can invalidate that item's path. |

Exact same-path single-item proposals retain `no_op=true` without an existing-destination issue. Other pair issues can still apply. Case-only or canonically equivalent existing-path proposals are conflicts, not a two-step rename.

The macOS adapter compares canonical NFD strings using Foundation's nonlocalized case-insensitive comparison. Original names are never normalized or rewritten. This is conservative dependency detection, not the selected volume's exact filename-equivalence rules; case-sensitive volumes can produce false positives and other filesystem/provider rules may differ. A clear issue list does not establish collision freedom or guarded-rename capability. Native string semantics are documented by Apple for [canonical decomposition](https://developer.apple.com/documentation/foundation/nsstring/decomposedstringwithcanonicalmapping) and [NSString comparison](https://developer.apple.com/documentation/foundation/nsstring).

## Observation boundary and remaining work

The batch anchors all item observations and rechecks to the initially observed canonical root, not a root symlink that could be retargeted between items. The top-level request preserves the supplied root spelling, including a supported root-directory symlink; each proposal's request uses the canonical root path. Every successful proposal must match the batch root's path, identity and version.

All initial successful single-item observations are rechecked after the batch scan, including root/source/parent and existing-destination revisions. The supplied root mapping is checked again before return. Changes during that recheck discard the aggregate rather than returning old entries as validated. An initial item error remains an observation of that failure; it is not retried or filled in automatically.

These are sequential observations, not an atomic namespace snapshot, path lock, reservation, authorization or reusable execution token. Changes after the last check remain possible; metadata versions are not content hashes or source-inode CAS. The no-materialization policy and filesystem-only provider boundary are inherited from single-item planning; true TCC/provider/unmount/read-only/external-volume and older macOS acceptance remain unverified.

Copy/duplicate support selected directory/package/link objects; different-volume move, existing-tag edits and confirmed trash/backup restoration are documented in the file-management references. Permanent deletion, dependency swaps/chains and automatic partial-operation reconciliation are excluded. Real File Provider/TCC/read-only/unmount/arbitrary external-volume and older macOS acceptance remains unverified.

Disposable tests cover exact names/order/versions, unchanged bytes/inode/mode/mtime/ctime, no-op/existing/broken-link targets, duplicate sources/destinations, hardlink identity, case/Unicode potential collisions across same/different parents, chains/swaps/nested directories, retained item errors, scope/count/byte bounds, alias/availability refusal, stable root-directory symlinks and retargeted-then-restored root symlinks, different-device observation, an earlier source/destination changing while later items are inspected and worker timeout. CLI tests exercise actual worker JSON, exit semantics, canonical-root anchoring, escaped names and no receipt announcement. No personal files or tag preferences are changed.
