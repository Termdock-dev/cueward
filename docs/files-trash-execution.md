# Verified single-file trash execution

`files trash execute` implements a deletion-protected slice of [#42](https://github.com/Termdock-dev/cueward/issues/42). Execution was merged in PR #61; check installed command help. [Trash planning](files-trash.md) remains read-only and never authorizes execution. Separate [backup restoration](files-trash-restore.md) supports completed receipts; permanent deletion is not implemented.

## Explicit selection and confirmation

Observe the exact file and its existing parent, present the selected path to the user, and obtain explicit confirmation before execution:

```bash
cueward files info --root /Users/me/Documents --path Reports/old.txt
cueward files info --root /Users/me/Documents --path Reports
cueward files trash execute --root /Users/me/Documents --path Reports/old.txt \
  --expected-version '<fresh file version>' \
  --expected-parent-version '<fresh source-parent version>' --confirm
cueward files trash receipt --operation-id '<announced UUID>'
```

The root must be absolute and the path an explicit non-root relative entry. There is no implicit selection, glob expansion, recursion, overwrite, force, caller-selected Trash path, follow-links, batch, resume, retry, restore or permanent-delete option. `--confirm` is required by both CLI and worker request validation. An earlier plan or execution receipt is not confirmation for another operation.

Trash supports available files, directory/package trees, opaque Finder aliases and symlink objects. It rejects unknown type/availability, hardlinked ordinary files, special entries, special permissions and compressed sources. Backup copying is bounded and verifies every descendant; symlink targets and alias targets are never opened. Source and existing private current-user Trash must share a filesystem, while selected root/receipt storage need not. Payload backups use same-source-volume private staging when needed; storage must be outside the selected source tree and Trash. Missing/inaccessible Trash or unsupported guarded rename is a structured refusal, without creating Trash or substituting permanent deletion.

## Backup, quarantine and native trash

1. A fresh UUID receipt is saved under `~/.cueward/operations/files/<id>/receipt.json`, then its ID is flushed to stderr. The supervised worker claims it once; it never resumes a progressed receipt.
2. Source/root/parent revisions and held descriptors are checked. Foundation locates an existing user Trash with `create=false`; its canonical path, private mode, owner, availability and same-volume descriptors are checked. No Trash enumeration occurs.
3. The selected object is copied into private same-source-volume storage as `staged-object`. SHA-256, byte count, Unix permissions, mtime and bounded xattrs are verified, then the copy is reopened and read back. The verified backup is retained even after success. ACLs, ownership and birth time are not promised on the independent backup; read access times may change.
4. A private `trash-source` directory is created. A guarded exclusive descriptor-relative rename moves the selected relative path into it under its original filename. The same inode, data/hash, permissions, mtime and xattrs must match the selected source and backup before any native trash request. A mismatched same-path replacement stays in private staging; it is not sent to Trash.
5. Foundation [`trashItem(at:resultingItemURL:)`](https://developer.apple.com/documentation/foundation/filemanager/trashitem(at:resultingitemurl:)) operates only on that verified private object. The returned actual URL is saved, rather than guessing `~/.Trash` or the resulting name. Native name-conflict handling can rename the item. Native errors are conservatively uncertain because side effects are not inferred from an error code.
6. The actual result must be in the previously verified Trash directory and match the original inode, readable content/hash, size, mode, mtime and original xattrs. Original and private staging paths must be absent, the backup unchanged, and namespace/descriptor checks stable before `completed`.

Native Trash may add `com.apple.macl` to the selected root object. Verification permits a newly added bounded marker, including the observed 72-byte value, only when removing that marker from the comparison restores the exact original attribute digest/byte count. The full actual digest/bytes and accepted marker size are recorded. Changed/removed preexisting MACL, changed original attributes and other additions are rejected. No attribute is removed or rewritten. Child attributes of directory trees remain exact. Observed revision changes during verification still produce incomplete evidence, not an automatic retry.

The scope-removal step uses Darwin `RENAME_EXCL | RENAME_NOFOLLOW_ANY | RENAME_RESOLVE_BENEATH` between held root/private-directory descriptors, including a private capability probe. There is no cached source-parent rename fallback. Source revisions are prechecks, not atomic source-inode CAS: a replacement between the final check and rename can be moved into private staging, but post-checks keep it there as incomplete evidence. Root/parent/Trash observations are sequential, not isolation. Native Foundation trash is path-based within the private current-user operation area; it is not an atomic identity lock against another process with the same user's privileges. Native/provider/TCC effects cannot be undone by killing the worker.

Native trash metadata may describe the private staging location. `finder_put_back_supported=false` and `recovery_supported=false`; do not use Finder's Put Back as restoration to the original selected path. The separate [backup restoration command](files-trash-restore.md) copies the verified backup to that original path without consuming Trash or backup. These false fields remain historical execution evidence, not installed-feature discovery. Do not delete operation directories or backups as routine cleanup.

## Result and interruption

External operation `trash` returns the receipt even for known failure; outer `Ok` is not completion. Exit zero requires both `status=completed` and `completion_verified=true`. `trash_receipt` / exit zero means saved evidence was read, not that the operation completed or its observations are still current.

Evidence includes request/confirmation, original/root/source-parent/Trash observations, backup path/independent identity and verification, private staging path/observation, actual native Trash path/observation, attribute verification, source/staging absence observations, `mutation_attempted`, `source_removed`, `trash_attempted`, `moved_to_trash`, stage, bounded error and final status. `trash_path=null` means no actual native location was saved, not proof that Trash is empty or untouched.

| Status | Meaning |
| --- | --- |
| `completed` | Backup and actual native trash result verified within the stated scope. |
| `not_started` | No source namespace removal is known to have occurred. A private backup/directory may remain. |
| `incomplete` | Removal to private staging or native trash is known, but subsequent checks failed. Retain all objects. |
| `uncertain` | Transport/lifetime/deadline failure, unknown removal outcome or native API error. Inspect original, private staging, backup and Trash before another decision. |

Stages are `preflight`, `backing_up`, `backed_up`, `quarantining`, `quarantined`, `trashing`, `verifying`, `finished`. A receipt checkpoint is last saved evidence, not proof a native operation is still running or has not advanced. Parent EOF/unexpected input terminates the worker; the whole-worker deadline kills its process group. If the parent survives transport failure it saves `uncertain`; parent death can leave the initial conservative status and last checkpoint. No automatic rollback, retry, restoration, permanent deletion or backup cleanup occurs.

`--max-bytes` defaults to 64 MiB, range 1..256 MiB, and bounds observed source size and backup transfer. Xattrs are bounded to 64 KiB names / 4 MiB aggregate values. `--timeout-ms` defaults to 10000, range 1..30000 for the whole worker. Requests fit 16 KiB; preflight evidence fits 64 KiB, working evidence 128 KiB, and the store caps receipts at 256 KiB, all before external escaping/envelopes. Oversized preflight observations are explicitly omitted on refusal. Large copies/readbacks can time out; do not automatically retry or extend a deadline after an uncertain removal.

## Verification scope and remaining work

Disposable tests cover native guarded quarantine, independent readable backup/xattrs, exact Unicode/newline names, stale guards, confirmation/limits, unavailable roles, hardlinks, parent relocation/symlink races, same-path replacement retained before native submission, backup/source/result edits, checkpoints, replay, schema separation, malformed framed requests, parent signals and deadlines. Controlled Trash hooks use owned private directories, not personal Trash. A separately ignored native CLI acceptance test operates on one owned fixture, verifies the real Foundation result and moves that exact fixture back manually; it never enumerates/empties Trash. Its macOS 27.0.1 result does not establish Finder UI/Put Back, real TCC denial, provider coordination, external/read-only/unmounted volumes or older macOS support.

`provider_coordination=filesystem_only_provider_state_unknown`; no implicit download or sync guarantee. Native owned file, nested directory, package and broken-link trash-to-backup-restore pairs passed on macOS 27.0.1 on 2026-10-03. One earlier native file fixture remained incomplete after its held-descriptor check failed during verification, despite matching original attributes plus a new 72-byte MACL; its original and verified backup were retained. Actual TCC/provider/read-only/unmount, arbitrary external-volume Trash and older macOS remain unverified. Permanent deletion is excluded.