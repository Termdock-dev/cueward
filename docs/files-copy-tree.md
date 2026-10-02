# Read-only recursive copy plans

`cueward files copy-tree plan` implements the directory-planning slice of [issue #42](https://github.com/Termdock-dev/cueward/issues/42). Check installed `--help` first. Recursive copying and directory duplication are not executable yet; existing [single-file copy/duplicate](files-mutations.md) commands retain their contracts.

## Select and observe

Select one absolute root, a relative source directory and an exact new relative destination beneath an existing directory. Observe the source and destination parent using `files info`, then provide both revisions:

```bash
cueward files info --root /Users/me/Documents --path Project
cueward files info --root /Users/me/Documents --path Archive
cueward files copy-tree plan --root /Users/me/Documents --path Project \
  --destination Archive/Project-copy --expected-version '<Project version>' \
  --expected-parent-version '<Archive version>'
```

There is no execute, overwrite, merge, follow-links, auto-suffix, parent creation, receipt lookup or saved-plan replay option. Planning does not authorize a future write. It does not edit payloads or selected metadata, create the destination, stage a copy, allocate mutation receipts, activate Finder or request downloads. Read-only transport can use private temporary files elsewhere; directory access times are not promised unchanged.

## Observations and blockers

The external JSON operation is `copy_tree_plan`. Outer `Ok` and exit zero only establish that observations returned. Inspect `has_blockers`, `issues`, every entry's `error` and `enumeration_complete`. `execution_supported` is always false, including plans without blockers. An empty blocker list does not certify full copyability, readability of every payload, free space, ACL preservation, filesystem publication support or a future operation's success.

Entries appear in sorted depth-first order, including the source directory (`relative_path="."`), hidden nodes and empty directories. Each entry retains `source` identity/version/metadata and its exact proposed relative `destination`. Available ordinary nodes include resource metadata and the existing copy-source metadata checks. Errors are retained for unsupported nodes rather than silently dropping them. Hardlinked regular files remain separate path entries; `observed_file_bytes` counts each observed file size, not unique inode storage, content hashes or bytes streamed. Unsupported regular-file sizes also count. Sizes under skipped directories are unknown and excluded.

Source must be a directory. Selected directories recognized as packages, aliases/unknown alias state, unknown package state, placeholders/unknown availability and unsupported metadata are not traversed; they have entry errors and `enumeration_complete=false`. Links and special files have entry errors and are never followed or opened as payloads; their presence alone does not mean directory enumeration was partial. Package ancestry is not pruned for an explicitly selected root or paths into package internals. Unknown/blocked destination-parent availability instead returns outer `Err`.

Top-level issues:

| Issue | Observation |
| --- | --- |
| `existing_destination` | A target exists, including a broken leaf symlink; no overwrite or merge is proposed. |
| `destination_within_source` | Destination parent has the source directory in its identity-based ancestry, including alternate case spellings on case-insensitive volumes. |
| `different_filesystem` | A supported source node and the selected destination parent report different devices. There is no cross-volume execution. |

Metadata checks reuse available-source alias/security/compression and bounded xattr validation. They do not read data forks, verify content hashes, preserve hardlink relationships, certify directory ACL inheritance or establish provider coordination. Per-field Foundation errors are not invented as empty values; unsafe alias/package states block traversal.

## Bounds and consistency

| Bound | Default | Accepted range |
| --- | --- | --- |
| `--max-entries` | 256 | 1..256, including top source and unsupported nodes |
| `--max-depth` | 16 | 1..32, with source at depth zero |
| `--max-bytes` | 67,108,864 | 1..268,435,456 observed regular-file size bytes |
| `--timeout-ms` | 10,000 | 1..30,000 for the entire worker |

Serialized requests must fit 16 KiB. Plans have a fixed 1 MiB pretty-JSON budget before external escaping/envelopes. Exceeding an entry/depth/byte/JSON limit returns outer `Err` with `scan_limit`, never a partial apparently successful manifest. Not every 256-node tree fits the other limits. Payload/native metadata allocation and enumeration latency are distinct from these result bounds. Supported nodes retain descriptors until final checks; descriptor exhaustion returns an error, not a partial plan.

All resolution uses the initially selected canonical root; the original supplied root spelling stays in the request and is rechecked at return. Directory names are enumerated from held descriptors using bounded `fdopendir/readdir`, not from a cached pathname that might now name a replacement directory. Available nodes use guarded no-symlink opens. Supported nodes are checked against their descriptors and path revisions; all retained unsupported node revisions are also rechecked. Destination parent and target revision/absence are rechecked before returning. Detected changes discard the plan.

These checks are sequential observations, not an atomic tree snapshot, namespace lock, reservation or execution token. External actors can change files after their last check or change provider state without a detectable filesystem revision. Reobserve before any later operation; never implement recursive copying by looping over existing single-file copy commands using this saved plan.

## Verification and remaining work

Disposable tests cover hidden/Unicode/newline names, sorted order, empty directories/files, selected hardlinks, unchanged selected bytes/inode/mode/mtime/ctime, symlinks/FIFO, package pruning, existing targets, identity-based ancestry, stale revisions, source/parent/target changes during later inspection, stable/retargeted root aliases, unknown availability and device mismatch seams, entry/depth/byte/JSON bounds, native descriptor enumeration and whole-worker timeout. Actual CLI tests verify envelopes, escaping, exit semantics and absence of receipt announcements. Invalid UTF-8 enumeration is tested through the platform seam because the local APFS volume rejects creating such names.

Native APIs were checked against the installed macOS 27 SDK and Apple's [Libc enumeration implementation](https://github.com/apple-oss-distributions/Libc/blob/main/gen/FreeBSD/readdir.c) and [directory entry definition](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/dirent.h). The adapter uses the already resolved `libc` package directly for platform bindings rather than duplicating Darwin ABI structures; no new package version is resolved.

Recursive-copy execution, directory/package duplicate, trash/recovery, cross-volume operations and safe existing-tag edits remain subsequent work. Real provider/TCC/read-only/unmount/external-volume and older macOS acceptance are unverified. Permanent deletion is excluded. Remaining work is tracked in #42.
