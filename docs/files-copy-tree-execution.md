# Verified recursive directory copy execution

`cueward files copy-tree execute` adds the bounded directory-copy slice of [issue #42](https://github.com/Termdock-dev/cueward/issues/42). This source capability was merged in [PR #59](https://github.com/Termdock-dev/cueward/pull/59); check installed command help. The separate [read-only plan](files-copy-tree.md) remains an observation, never a saved execution token. Existing single-file copy/duplicate commands keep their contracts.

## Select a fresh authorized request

Select one absolute root, a relative available ordinary source directory and one exact new relative destination under an existing parent. Observe both revisions immediately before the authorized operation:

```bash
cueward files info --root /Users/me/Documents --path Project
cueward files info --root /Users/me/Documents --path Archive
cueward files copy-tree execute --root /Users/me/Documents --path Project \
  --destination Archive/Project-copy --expected-version '<Project version>' \
  --expected-parent-version '<Archive version>'
cueward files copy-tree receipt --operation-id '<returned or stderr-announced ID>'
```

For a sibling directory copy, select an explicit new destination such as `Project-copy` and observe its existing parent (`.`). `files duplicate` separately supports exact sibling tree copies; neither command chooses Finder-style names. There is no overwrite, merge, auto-suffix, follow-links, missing-parent creation, plan-file input, resume, retry, rollback, cleanup or permanent-delete option. Source payloads are never edited or removed; access times are not preserved.

Hidden files, empty files/directories and available ordinary nested directories are included. Each regular-file path, including a hardlink path, becomes an independent destination inode. All source nodes must pass the existing resource/type/metadata checks. Packages are blocked by default; [explicit package inclusion](files-package-copy.md) permits known package directories when installed. Special files, unknown alias state, unknown package state, placeholders/unknown availability, special permission bits and compressed sources block execution; unsupported nodes are not skipped. Explicit paths inside a package are not pruned by package ancestry. An existing destination (including a broken leaf link) or destination-parent ancestry inside the source blocks execution.

## Private construction, then one publication

The worker obtains a fresh bounded plan, holds source/root/parent descriptors and records bounded xattr digests. Before each staged creation or directory metadata write it rechecks the root mapping, root/parent revisions and the selected node's path/descriptor. Full tree revision/attribute checks run before publication and during post-publication verification. All directory enumeration uses guarded descriptors.

The complete tree is constructed in a new private operation directory under `~/.cueward/operations/files/<UUID>` on the receipt storage filesystem. Staging uses a destination-local `.cueward-stage-<operation-id>` directory if receipt storage is on another filesystem. Storage inside the selected source tree is refused before staged payload creation. Each file uses the existing verified copy engine: bounded bytes, SHA-256, xattrs, permissions and mtime, with read-back. Directory xattrs, permissions and mtime are restored bottom-up, and exact child name sets and all staged paths/descriptors are verified.

Only then does one native guarded exclusive rename publish the tree under the held selected root. No destination payload or metadata write follows publication. The worker reopens every published path, checks identity/revision, file hashes, xattrs, permissions/mtime, directory child sets and source/root/parent consistency before recording completion. Publication can change the top destination directory's ctime; its held identity and final revision are checked, while child revisions must match staging.

This provides a single target namespace publication, not a transactional filesystem snapshot or atomic source-version/inode comparison. Sequential checks can detect observed changes but do not lock out other actors. A destination parent replaced inside the root during native submission can receive a tree and then fail verification; the receipt is incomplete, not completed. A cached parent moved outside the selected root is not used for publication. Crash durability, concurrent-provider semantics and full Finder metadata equivalence are not promised. ACLs, ownership/group, birthtime, flags, sparse layout and hardlink relationships are not promised preserved.

## Bounds

| Bound | Default | Accepted range |
| --- | --- | --- |
| `--max-entries` | 64 | 1..64, including the top source directory |
| `--max-depth` | 16 | 1..32, source at depth zero |
| `--max-bytes` | 67,108,864 | 1..268,435,456 summed observed regular-file sizes and bounded transfers |
| `--timeout-ms` | 10,000 | 1..30,000 for the whole worker, including receipt checkpoints |

The read-only plan still accepts 256 entries. Execution is stricter to retain bounded per-node and final evidence in the unchanged 256 KiB private receipt store. Initial pretty-JSON evidence must fit 64 KiB, encoded identity/revision fields 512 bytes each and each full retained FileInfo 4 KiB. If initial evidence cannot fit, execution refuses before staging, clears oversized node/before observations and records their count in `preflight_nodes_omitted`. That count does not represent an executed partial tree. Errors are clipped to 1,024 UTF-8 bytes with `error_truncated=true` when necessary.

Xattr values have an additional 8 MiB aggregate tree preflight limit, on top of the existing 4 MiB values/64 KiB names per node. Requests fit 16 KiB; the fresh plan must also fit its 1 MiB budget. A count within 64 does not guarantee fitting paths, metadata, bytes, descriptors or the deadline. These bounds are not a guarantee of native peak allocation under concurrent metadata changes. Parent-side receipt preparation/finalization is outside the worker deadline.

## Receipts, interruption and completion

External JSON operation is `copy_tree` for execution and `copy_tree_receipt` for typed lookup. Execution exits zero only when `status=completed` and `completion_verified=true`; outer `Ok` can contain a failed receipt and a nonzero exit. Lookup exits zero when saved evidence was loaded, regardless of the saved operation status. Generic `files receipt` does not interpret tree receipts.

| Status | Meaning |
| --- | --- |
| `completed` | Published tree and final source/destination postconditions were verified before saving completion. |
| `not_started` | Requested destination creation was rejected or known not attempted. Private partial/verified staged content can still remain. |
| `incomplete` | Destination creation is known, but a later check/checkpoint failed. Inspect that destination; do not repair or delete automatically. |
| `uncertain` | Saved evidence cannot establish current delivery/completion, including worker interruption, timeout or lost/invalid response. |

`stage` is progress (`preflight`, `staging`, `copying`, `metadata`, `staged`, `publishing`, `verifying`, `finished`), not current delivery proof. `active_index` identifies the checkpointed node. `nodes` contains source identity/version and attribute digests, file verification when available, and destination identities/revisions when recorded. Directories intentionally have no payload hash. `mutation_attempted` refers to destination publication, not private staged copying. After interruption, evidence can lag actual writes; even a false attempt flag does not justify a blind retry. `staging_verified=true` does not mean published or completed.

The parent announces the operation ID on stderr before dispatch. Workers are claimed once and monitor the parent's socket lifetime before any work and continuously thereafter. Parent SIGINT/SIGTERM/SIGKILL stops workers, and worker deadlines stop their process group. Saved lookup never reopens selected paths or resumes execution. Inspect the receipt, private staging, source and destination before deciding another explicitly authorized operation. Private retained trees can contain user data and consume space; no automatic retention cleanup is included.

Current copy/duplicate, different-volume moves, existing-tag edits and directory/link trash/restore are described in [file mutations](files-mutations.md), [relocation](files-relocation.md), [tags](files-tags.md) and [trash](files-trash-execution.md). File Provider/TCC/read-only/unmount, arbitrary external volumes and older macOS remain unverified. Permanent deletion is excluded.

## Verification scope

Disposable native tests cover nested/hidden/Unicode/newline names, empty trees/files/directories, hardlink independence, file/directory xattrs, permissions/mtime, existing targets/broken links, packages/unsupported nodes, stale revisions, changed source/root mappings, checkpoint failures, late conflicts, moved/replaced parents, unknown publication outcome, post-publication edits, same-store overlap, 64-entry execution, aggregate xattrs and initial receipt overflow. Process tests stop armed and partially copying workers with parent SIGINT/SIGTERM/SIGKILL and a real worker deadline. CLI/worker tests cover external escaping, typed lookup, replay rejection, exit semantics and closed-parent rejection. These fixtures do not establish the unverified platform/provider scenarios above.
