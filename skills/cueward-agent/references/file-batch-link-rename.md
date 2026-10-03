# Rename explicitly selected link objects in a batch

New per-entry `link_itself` JSON support is merged in PR #65. Check installed `files rename-batch plan/execute --help` for that field; single-item `--link-itself` support is insufficient. Read [batch rename](file-batch-rename.md) for authorization, scope, bounds, fail-stop execution and receipt semantics, and [link-object relocation](file-link-relocation.md) for the target boundary.

Observe each link itself with `files info` without `--follow-links`, plus its current parent. Serialize the user's explicit entries:

```bash
cueward files rename-batch plan --root /absolute/directory \
  --entry '{"path":"link","name":"renamed-link","expected_version":"<link version>","expected_parent_version":"<root version>","link_itself":true}' \
  --entry '{"path":"report.txt","name":"report final.txt","expected_version":"<file version>","expected_parent_version":"<root version>"}'
```

Planning never authorizes writes. Reobserve after changes; only for an authorized batch use `rename-batch execute` with fresh versions and the same explicit per-entry permission. There is no global `--link-itself` on batch commands. Missing/false still rejects source links; true requires a symlink and rejects ordinary entries. One invalid entry blocks the entire preflight. Mixed permitted links, ordinary entries, separate parents and exact same-path no-ops retain existing bounds and conflict/dependency rules. No overwrite, ordering, retry, resume or rollback is available.

Reference text is opaque UTF-8, including broken/outside/relative references. Targets are never followed, opened, queried, downloaded or edited; resources are `not_applicable`. The same inode/exact reference text is retained. Each link stays in its current parent, but other selected renames can change reference meaning; do not promise reachability or repair. Independent unselected target edits do not invalidate the link-object guard.

Inspect every plan error/issue; `execution_supported=false` remains true of all read-only plans. Execution succeeds only with aggregate `status=completed` AND `completion_verified=true`. Use `files rename-batch receipt --operation-id '<aggregate UUID>'` and `files relocation receipt --operation-id '<child UUID>'`. True is saved in aggregate entries and child requests; false is omitted. Parent revision advancement does not drop permission. A later aggregate failure can coexist with verified children; interruption can leave submitted progress and unallocated pending items. Preserve IDs and inspect actual paths without automatic retry or deletion.

Use the same compatible binary for both lookups. Old batch entry decoders reject unknown `link_itself`, including aggregate receipts, instead of silently dropping it. Pre-#64 child relocation readers can still omit the flag while exiting successfully; their output is incomplete evidence. Preserve raw `receipt_path` JSON and never strip fields, migrate namespaces or replay to make an older reader accept it. New readers accept legacy omission as false.

Sequential checks are not source-inode CAS, an atomic batch or provider coordination. Dependency swaps/chains remain rejected. Copy/duplicate/trash and cross-volume move support link objects separately, never target resolution.