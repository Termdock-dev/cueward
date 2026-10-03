# Restore a retained verified trash backup

Source capability merged in PR #62. Check installed `files trash restore --help` and `files trash restore-receipt --help`. Use for an explicit user request to restore the original verified backup of a completed verified trash operation. This is an independent backup copy, not native Trash movement or Finder Put Back. Never use it to reconcile an uncertain/incomplete trash receipt.

Read the original [trash receipt](file-trash-execution.md) and observe its original parent after trashing:

```bash
cueward files trash receipt --operation-id '<original trash UUID>'
cueward files info --root /absolute/directory --path Reports
cueward files trash restore --operation-id '<original trash UUID>' \
  --root /absolute/directory --expected-parent-version '<fresh Reports version>'
cueward files trash restore-receipt --operation-id '<new announced restore UUID>'
```

For a root-level file observe `.`. Restore uses the original trash UUID; restore-receipt uses the new restore UUID announced on stderr. The explicit root must match the original canonical root and identity; the original parent must still have its recorded path/identity and match the fresh supplied version. Destination is derived only from the original receipt. Any existing file, directory or broken link blocks publication. No new parents, alternate destination, overwrite, force, link following, retry, resume or cleanup are supported. No deletion-confirmation flag is needed for this copy, but a receipt alone is not authorization.

The retained backup must match its recorded path, identity/version, content hash, bytes, Unix permissions, mtime and bounded full xattrs. It is copied into a new private operation and verified before guarded exclusive publication at the original path. The restored file has a new identity. Owner/group/ACL/birth time/flags/access time and provider synchronization are not guaranteed. Reading can change access times. Root/parent/backup/candidate checks and original-receipt consistency checks are sequential, not isolation or identity CAS.

Native Trash is not queried, moved, consumed or cleaned. Original backup and trash receipt are retained unchanged. A changed/missing native Trash item does not change the restored content: only the saved original verified backup is used. The original receipt's `recovery_supported=false` is historical trash-execution evidence; check installed help for this separate capability. `finder_put_back_supported=false` remains valid because native metadata may point at private staging.

Operation `trash_restore` can return outer `Ok` with a nonzero exit. Require `status=completed` AND `completion_verified=true`. `not_started` means original-path publication was not attempted or known refused, but private copied data can remain; `incomplete` means known publication followed by failed checks; `uncertain` means unknown delivery or supervision/transport failure. `trash_restore_receipt` only loads saved evidence. Keep both UUIDs, receipt paths, original-receipt fingerprint, destination/staging paths, stage/error, backup/candidate proofs, `mutation_attempted` and optional `destination_created`. Null means unobserved, not absent. The fingerprint is a consistency hash, not an authenticity signature.

Parent death/deadlines stop local execution without undoing already-published copies. A last `publishing` checkpoint with `destination_created=null` can coexist with a restored file. On a non-completed result, stop and inspect original destination, private staging and original backup before a new user decision. Do not automatically retry, overwrite, roll back, remove evidence/backups or empty Trash. `trash_item_touched=false`, `backup_removed=false` and `recovery_method=verified_backup_copy` describe this non-destructive method; receipts are not fresh observations.

Owned macOS 27.0.1 file/directory/package/link native trash-to-restore pairs passed on 2026-10-03. A newly added bounded MACL marker can be accepted only when every original attribute remains exact; the full actual attributes are recorded. Revision changes still produce incomplete evidence. Provider/TCC/read-only/unmount/arbitrary external-volume Trash and older macOS remain unverified. Preserve evidence and never automatically replay an uncertain removal.