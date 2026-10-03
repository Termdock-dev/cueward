# Restore a verified retained trash backup

`files trash restore` adds non-destructive backup recovery for [#42](https://github.com/Termdock-dev/cueward/issues/42). This source capability is pending review; check installed command help. It accepts only a completed, verified ordinary-file [trash execution receipt](files-trash-execution.md). It copies the retained backup to the recorded original path, with a new identity, without overwriting anything. It does not move, inspect or remove the native Trash item, consume the backup or rewrite the original receipt.

## Explicit request

Read the original trash receipt and observe its original parent after trashing. The parent version from before trashing is stale. For a root-level file, observe `.`:

```bash
cueward files trash receipt --operation-id '<original trash UUID>'
cueward files info --root /Users/me/Documents --path Reports
cueward files trash restore --operation-id '<original trash UUID>' \
  --root /Users/me/Documents --expected-parent-version '<fresh Reports version>'
cueward files trash restore-receipt --operation-id '<new announced restore UUID>'
```

`--operation-id` on restore names the original trash operation; lookup names the new restore operation. Root is explicitly supplied and must resolve to the recorded original canonical root and identity. Destination comes only from the original receipt's relative path. The existing original parent must retain its recorded canonical path and identity and match the fresh supplied version. Missing parents are not recreated. Existing files, directories and broken links are conflicts. There is no alternative destination, overwrite, force, recursive, follow-links, retry, resume or cleanup option. Restore creates an independent copy and needs no deletion-confirmation flag; the user's explicit restore request is still required.

Uncertain/incomplete trash receipts, missing evidence and unsupported schemas are refused without copying. This command does not reconcile partial trash operations. The backup must remain at that original operation's `staged-object`, with its recorded identity/revision, size, mode, mtime and full data/xattr verification. Replacement, symlink retargeting, editing or a changed original receipt fails closed. A canonical SHA-256 of typed original receipt JSON records consistency, not authenticity against a same-user editor.

## Copy and no-overwrite publication

A new private 0700 operation directory and receipt are allocated under `~/.cueward/operations/files/<new UUID>`. Its ID is flushed to stderr before one-use supervised worker dispatch. After preflight, the backup is copied into that new operation's `staged-object` and read back, verifying SHA-256/bytes, Unix permissions, mtime and bounded full xattrs against the original backup proof. The source backup is held and reobserved throughout. The original receipt is reloaded and compared at checkpoints. New storage must be outside the selected root, separate from the original backup operation and on the same filesystem.

Publication uses the existing guarded exclusive root-relative Darwin rename, with no cached-parent write fallback. Root/parent mappings, held descriptors, destination absence, backup and candidate are checked before publication and after it. Completion requires readable verified content at the original path, a stable original root/parent mapping and an unchanged verified backup. The restored file has a new inode; owner/group, ACLs, birth time, access time, flags and cloud synchronization are not promised. Reading may change access times.

Native Trash is not accessed during restore. It can have moved, changed or been emptied since trash execution; restore still uses only the original verified backup, not subsequent Trash edits. The original Trash receipt's `recovery_supported=false` remains historical execution evidence, not installed-feature discovery. `finder_put_back_supported=false` still applies: native Put Back may target private staging, not the original path.

Namespace/descriptor observations are sequential, not isolation, parent-identity CAS or protection against another same-privilege process. Guarded publication prevents overwrite, link following and escape through destination components. A parent/root move or replacement at submission can still cause an incomplete post-check outcome. No automatic repair, deletion or rollback follows.

## Receipts and interruption

Operations are `trash_restore` and `trash_restore_receipt`. Execution can return outer `Ok` with nonzero exit; zero requires both `status=completed` and `completion_verified=true`. Lookup success means saved evidence was read, not completion or current filesystem state. Trash and restore receipt schemas are separate.

| Status | Meaning |
| --- | --- |
| `completed` | Independent original-path copy and final backup/scope checks verified. Trash and original backup retained. |
| `not_started` | Destination publication was not attempted or known refused. A private partial/verified candidate can remain. |
| `incomplete` | Destination publication is known, but subsequent verification or a checkpoint failed. Preserve actual objects. |
| `uncertain` | Unknown publication outcome or worker/transport/lifetime/deadline failure. Inspect before a new decision. |

Stages are `preflight`, `copying`, `staged`, `publishing`, `verifying`, `finished`. `mutation_attempted` refers to publication, not private copy writes. Evidence includes original trash UUID/fingerprint, recorded original source/destination, root/parent/backup before and after, private staging identity/verification, optional `destination_created`, destination after, bounded error and completion. `recovery_method=verified_backup_copy`, `trash_item_touched=false` and `backup_removed=false` describe the implemented method. Null observations mean unobserved, not absent. A checkpoint can lag actual writes.

Parent death stops the worker continuously; the whole-worker deadline stops its process group. A surviving parent saves conservative `uncertain` on lost/invalid response. A destination can exist even if the last saved `destination_created` is null. Never automatically retry, delete staged content, remove backups/receipts or empty Trash. Retained copies consume disk space and have no automatic expiry.

Timeout defaults to 10000 ms, range 1..30000, excluding parent receipt setup/finalization. Requests fit 16 KiB; preflight evidence fits 64 KiB, working evidence 128 KiB and the unchanged store limit is 256 KiB before external escaping. Errors are bounded to 1024 UTF-8 bytes. Copy bytes are bounded by the saved backup proof, which cannot exceed the original trash transfer bound (at most 256 MiB). Existing xattr limits are 64 KiB names / 4 MiB values. Oversized preflight observations are omitted on refusal, not treated as a completed partial restore.

## Verification scope

Disposable tests cover backup-based recovery, unchanged original Trash/backup/receipt, independent identities, nested Unicode/newline names and xattrs, changed/missing Trash items, stale parent and wrong root, unsupported original evidence, changed backup identity/data/attributes, conflicts including broken links, missing/replaced parents, staged edits and late namespace retargeting/collisions. Worker tests cover one-use claims, role/schema separation and original-receipt changes. Real CLI tests cover framing, external envelopes, nonzero refused results and saved lookup. Parent SIGINT/SIGTERM/SIGKILL and deadline tests interrupt armed, partially copied, pre-publication and already-published workers without cleanup.

The separately ignored native CLI acceptance test trashes one newly owned fixture with Foundation, restores through this command and verifies that its exact native Trash entry, backup and original receipt remain unchanged. Test cleanup touches only that owned entry after verifying its restored copy; it never enumerates or empties personal Trash. The 2026-10-03 macOS 27.0.1 run was blocked before restore: native trash added a 72-byte `com.apple.macl`, and the existing empty-marker-only policy retained an `incomplete` trash receipt. That evidence is not accepted for restoration; this PR does not broaden the trash exception. A separate explicit release-CLI probe passed using a completed receipt from controlled owned-trash execution, exercising the actual restore worker, guarded publication, conflict refusal and saved lookup. Native trash-to-restore end-to-end acceptance, older macOS, true TCC denial, read-only/unmounted/external volumes, File Provider coordination and Finder UI remain unverified. `provider_coordination=filesystem_only_provider_state_unknown`; no download is requested.

#42 remains open for broader trash/recovery types, partial-operation recovery, cross-volume operations, package duplication, existing-tag edits and remaining platform acceptance. Permanent deletion is excluded.
