# Rename or move a symlink object

`files rename/move --link-itself` adds explicit leaf-symlink relocation for [#42](https://github.com/Termdock-dev/cueward/issues/42). This new flag is pending review; check installed help. It reuses the [same-volume relocation](files-relocation.md) worker, revisions, guarded exclusive native rename and receipts. Without the flag, initial source links remain rejected. Recursive copy, single-file copy, batch rename and trash do not gain link support.

## Observe the link, not its target

```bash
cueward files info --root /absolute/directory --path from/link
cueward files info --root /absolute/directory --path to
cueward files move --root /absolute/directory --path from/link \
  --destination to/link --expected-version '<link version>' \
  --expected-parent-version '<to version>' --link-itself --dry-run
# Reobserve before an independently authorized write.
cueward files move --root /absolute/directory --path from/link \
  --destination to/link --expected-version '<fresh link version>' \
  --expected-parent-version '<fresh to version>' --link-itself
cueward files rename --root /absolute/directory --path to/link --name renamed-link \
  --expected-version '<fresh link version>' \
  --expected-parent-version '<fresh to version>' --link-itself
cueward files relocation receipt --operation-id '<announced UUID>'
```

Use the link's own `files info` version without `--follow-links`. Root is absolute; source and exact destination are relative normal-component paths. The destination parent must exist. `--link-itself` requires a symlink source, rejecting ordinary files/directories rather than treating the flag as optional permission. Intermediate links in either path are still rejected. An existing target, including a broken leaf link, is a conflict; no overwrite or merge is added.

The link target is opaque UTF-8 reference text, not a second selection. It can be absolute, relative, broken, a loop or outside the root. It is never resolved, opened, queried through Foundation, downloaded or relocated. The source/parent observations and same-device checks describe the link and its destination parent, not the target's filesystem or availability. The source's resource fields are `not_applicable`, not invented available/false/empty values. Non-UTF-8 link text cannot be represented and is refused without lossy conversion.

The existing symlink inode and exact target text are retained. Relative references are **not rewritten**; moving a link to another parent can change what it resolves to. Completion does not promise that the reference still resolves or reaches the original target. Exact same-path selection verifies a no-op; case-only existing-path conflicts remain rejected.

## Native boundary and saved evidence

The adapter opens the already no-follow parent directory, then uses descriptor-relative `openat` with Darwin `O_SYMLINK`, holding the link descriptor without opening its target. It verifies the descriptor's symlink type and compares its metadata revision with the scoped observation. `O_NOFOLLOW_ANY` still guards the parent open; it cannot be combined with `O_SYMLINK` on the leaf because the tested kernel rejects that combination. No native APIs enter core and no dependency is added. The root-relative `renameatx_np` operation retains its existing `RENAME_EXCL | RENAME_NOFOLLOW_ANY | RENAME_RESOLVE_BENEATH` flags.

Preflight rejects stale link/parent revisions and revalidates held root/source/parent descriptors and paths. Post-call completion additionally compares exact link target text alongside selected identity/type/size/mtime/mode/birthtime and the held descriptor's current revision. This remains a sequence of observations, not source-inode CAS or protection against malicious namespace races. An external replacement after the last check can be moved instead; detected differences produce `incomplete` evidence and no rollback. Independent target edits do not invalidate a link-object operation.

Operations remain `rename_plan`, `move_plan`, `rename`, `move` and `relocation_receipt`. `link_itself=true` is retained in the request and saved evidence; missing/false preserves legacy rejection and false is omitted on serialization. Non-boolean values are rejected. Use a compatible installed CLI to operate links; saved lookup alone neither authorizes a write nor proves current completion. Execution exit zero requires `status=completed` and `completion_verified=true`; dry-run observations do not certify native support or grant an execution token.

Receipt storage, one-use claims, stderr operation announcements, parent SIGINT/SIGTERM/SIGKILL lifelines, timeout 1..30000 ms/default 10000 and conservative `not_started/incomplete/uncertain` outcomes are unchanged. Inspect receipt and actual paths on failure; do not retry, rewrite the reference, roll back, delete or replay automatically. No payload backup, activation, cross-volume copy/delete or target repair is added. Provider coordination remains `filesystem_only_provider_state_unknown`.

## Verification and remaining work

Owned macOS 27.0.1 fixtures cover selected absolute/outside, relative and broken references, source-default refusal, no target stream/resource calls, native held link descriptors, unchanged outside target bytes/revisions, link inode/reference preservation across move/rename/no-op, saved opt-in/legacy JSON, replay rejection, conflicts, non-link sources, stale versions, ancestor links, cross-device refusal seams, pre-submit/post-last-check link replacements, external target edits and delayed/armed parent interruption. Only owned disposable fixtures and exact newly created receipt directories are cleaned up; personal links/files/Trash are untouched. A release CLI/worker probe on 2026-10-03 passed dry-run, default refusal, same-inode move/rename/no-op, unchanged outside target bytes/revision, broken-reference retention, no-overwrite conflict and saved lookup.

These checks do not establish provider/TCC/read-only/unmount/external-volume behavior or older macOS support. #42 remains open for cross-volume operations, copy/tree symlink support, broader package/metadata support, safe existing-tag edits, broader trash/recovery and platform acceptance. Permanent deletion is excluded.

Native references: Apple's [symlink open option](https://developer.apple.com/documentation/system/filedescriptor/openoptions/symlink) describes opening the link object; the [XNU symlink-open test](https://github.com/apple-oss-distributions/xnu/blob/main/tests/vfs/open_symlink.c) exercises native link descriptors. Constants were checked against the installed macOS 27 SDK, and supported combinations were tested with owned links on macOS 27.0.1 rather than inferred from cross-platform `open` behavior.
