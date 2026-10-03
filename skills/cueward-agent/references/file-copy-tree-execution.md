# Execute an authorized bounded directory copy

Source capability merged in [PR #59](https://github.com/Termdock-dev/cueward/pull/59); this does not establish availability in the installed CLI. Check installed `files copy-tree execute --help` and `files copy-tree receipt --help`. Use only for the user's selected directory and exact new destination. A [read-only plan](file-copy-tree.md), revisions or a saved receipt do not authorize a write.

Observe source and the destination's existing parent with `files info`, then submit fresh guards:

```bash
cueward files copy-tree execute --root /absolute/directory --path Project \
  --destination Archive/Project-copy --expected-version '<Project version>' \
  --expected-parent-version '<Archive version>'
cueward files copy-tree receipt --operation-id '<returned or stderr-announced ID>'
```

For a sibling copy, choose an explicit new relative destination and observe its parent (`.` when appropriate). `files duplicate` also supports directory/package/link objects. There is no overwrite, merge, auto-name, follow-links, missing-parent creation, saved-plan input, retry/resume, rollback, cleanup or permanent-delete fallback. Never emulate recursive execution by looping over single-file commands using a saved plan.

The worker replans every node and refuses the entire copy on blockers. Files/directories, opaque aliases and symlink references are supported; links are never followed. Packages need explicit --include-packages; unknown state/special entries/metadata stay blocked. Existing destinations and destination ancestry inside the source are blockers. Different filesystems use destination-local private staging.

Execution accepts at most 64 entries including the source (default 64); read-only planning still accepts 256. Max-depth defaults 16/range 1..32, max-bytes defaults 67108864/range 1..268435456 summed file sizes and bounded copying; timeout-ms defaults 10000/range 1..30000 for the whole worker. Xattr values have an additional 8 MiB tree budget and existing 4 MiB values / 64 KiB names per-node bounds. Initial receipt evidence must fit 64 KiB within the unchanged 256 KiB store. Long paths or metadata can fail even within the entry limit. Initial evidence overflow clears oversized observations and reports preflight_nodes_omitted before staging, not a partial execution.

The tree is verified privately then published once with guarded exclusive rename. Destination-local staging supports another filesystem; storage inside the selected source tree is refused. Content/reference text, mode/mtime, xattrs and child sets are checked before/after publication. Originals remain intact; ACLs/ownership/birthtime/flags/hardlink relationships/provider synchronization are not promised.

Operation is copy_tree. Execution exits zero only for status=completed and completion_verified=true; outer Ok may carry a failed receipt with nonzero exit. Operation copy_tree_receipt / lookup exit zero only means saved evidence loaded. Generic files receipt rejects this schema. Interpret:

- completed: final published tree and source/destination checks passed before the completion checkpoint.
- not_started: requested publication rejected/not attempted; private staged data can still exist.
- incomplete: destination creation is known, but later verification/checkpoint failed.
- uncertain: interruption, timeout or lost response prevents establishing delivery/completion.

Stage and active_index are progress, not delivery proof. `staging_verified` does not mean completed. `mutation_attempted` refers to target publication, not private copying; a saved false flag can lag actual progress. Node verification is absent for directories, not a missing directory hash. `error_truncated` indicates a clipped diagnostic.

Preserve the announced operation ID. After any failure, inspect saved receipt, private staging, source and destination before deciding another authorized operation. Lookup does not inspect current selected paths, resume, reconcile or clean up. Workers are one-use, monitor parent lifetime and stop on parent death or deadline; saved evidence can lag actual writes. Retained private trees can contain user data and consume space. Do not blindly rerun, repair or delete them.

`provider_coordination`=filesystem_only_provider_state_unknown; no implicit downloads or provider sync guarantee. Cross-volume copying and existing-tag edits are not supported here. Explicit package contents have the bounded opt-in above, not universal bundle support. Confirmed selected-object trash uses [file-trash-execution.md](file-trash-execution.md); completed verified receipts have separate [backup restoration](file-trash-restore.md). Real provider/TCC/read-only/unmount/external-volume and older macOS acceptance remain unverified.
