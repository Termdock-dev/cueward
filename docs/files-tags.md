# Observed Finder tag names and atomic initial tagging

`files tags read/add/remove/receipt` implements Finder tag management for [#42](https://github.com/Termdock-dev/cueward/issues/42). Check installed help. Add/remove edit requested names on absent, empty or existing supported tag attributes, preserving unselected names, order and colors. A revision is an observation, not authorization.

```bash
cueward files tags read --root /Users/me/Documents --path report.txt
cueward files tags add --root /Users/me/Documents --path report.txt --expected-version '<file.version>' --expected-tags-version '<tags_version>' --tag Work --tag Review
cueward files tags read --root /Users/me/Documents --path report.txt
cueward files tags remove --root /Users/me/Documents --path report.txt --expected-version '<fresh file.version>' --expected-tags-version '<fresh tags_version>' --tag AlreadyAbsent
cueward files tags receipt --operation-id '<announced operation ID>'
```

The add example succeeds only when the tag attribute is absent. The remove example verifies a missing-name no-op; it cannot delete an existing tag. Use Finder for edits to existing attributes, including a stored empty attribute.

## Selection and supported representation

All commands require one absolute root and a relative existing path. Read defaults to `.`; edits require explicit `--path`, `--expected-version`, `--expected-tags-version` and at least one `--tag`. Each command has a 1..30000 ms worker deadline, default 10000. Symlinks, Finder aliases, unknown alias state, dataless/unknown availability and special files are rejected. No link following, download, replacement tag-set, color editing or automatic retry option is provided. Ordinary directories and package directories can be selected; package contents are not traversed or read. Hardlinked regular files can be read but edits are rejected because metadata changes would also affect unselected names.

Add appends missing exact names with stored color 0; remove deletes only requested existing names. Existing-name additions and absent-name removals are verified no-ops. Matching is case-sensitive without normalization/trimming. Names must be nonempty without NUL/CR/LF and fit 1024 UTF-8 bytes each, 256 requested names and 8192 aggregate request-name bytes. The result holds at most 256 names/16384 name bytes. Duplicate requests do not create duplicate tags.

The adapter supports the bounded binary-plist string-array representation observed on macOS 27.0.1 in `com.apple.metadata:_kMDItemUserTags`. Entries may be unsuffixed names or names with a stored newline plus color digit 0..7. Read/no-op requests retain existing names/color suffixes; initial new names use stored color 0. Snapshot `color=null` means no suffix was stored, not an unknown name or a queried Finder preference. This is not a guarantee of current Finder UI color, sidebar registration or global tag preferences. Apple documents tag names through NSURL, but does not establish this on-disk representation as a universal API contract; unsupported shapes are rejected instead of reconstructed or silently erased.

A missing tag attribute is established by descriptor `ENOATTR`, not by assuming a missing Foundation resource is an empty list. Its revision differs from a stored empty array. Native Foundation names must agree with decoded names; missing resource values are accepted only when the descriptor establishes a known empty set. Malformed/oversized attributes, unknown color suffixes, inconsistent native names and unsupported filesystems remain errors.

Legacy FinderInfo labels are not edited. A label-only representation whose native names disagree with the stored array is refused. Removing all stored tags is refused when it would expose a nonzero legacy label; adding/removing a subset is allowed while preserving FinderInfo and unrelated xattrs. This is not complete legacy-label synchronization.

## Write boundary and verification

The adapter holds a no-follow inode descriptor, saves the original raw attribute as `original_attribute_base64` before writing, and serializes Cueward editors with a nonblocking inode lock. File and exact tag-attribute revisions are checked immediately before `fsetxattr`; creation uses XATTR_CREATE and existing attributes use XATTR_REPLACE. Creation cannot overwrite an externally added attribute; replacement cannot recreate an externally removed attribute. Final stored names/colors and native Foundation names are verified.

After submission, the worker verifies stored names/colors, a fresh Foundation name view, target/root identity, target size/mtime/mode and final descriptor/path revisions. Detected relocation, replacement or read-back failure prevents `completed`. Content is not hashed because tag operations do not read or write payload bytes; preserving content bytes was checked separately with owned fixtures. Receipts do not prove current/future state, cloud synchronization or Finder refresh.

Existing-value replacement is not atomic compare-and-swap: Finder/provider editors do not honor the Cueward lock, and an edit between the last check and setter can be overwritten. Observed external changes stop submission; later changes cause incomplete verification where detected. A descriptor write can still affect the selected inode if another actor relocates it. Do not promise isolation, future state, Finder UI refresh or provider synchronization, and never automatically restore the saved attribute over newer edits.

## Receipts, interruption and retention

External JSON is `Ok` with operation `tags_read`, `tags_edit` or `tags_receipt`, or structured `Err`. Paths, names and diagnostics are escaped within the shared `<external>` envelope. Add/remove exit nonzero unless status is `completed` and `completion_verified=true`; outer Ok alone does not establish success. Receipt exits zero when evidence was read, regardless of its status.

- `completed`: requested edit or no-op and final observations verified.
- `not_started`: known non-submission/rejection.
- `incomplete`: a successful native write followed by failed verification/checkpoint.
- `uncertain`: delivery/completion cannot be established; tags may have changed.

Receipt fields include operation_id, request, receipt_path, status/stage/error, mutation_attempted, changed_by_operation, completion_verified, before/after snapshots and original_attribute_base64. The original attribute is saved before submission for authorized manual recovery; null with a before snapshot records proven absence, while null without a before snapshot establishes nothing. There is no automatic restore or deletion. Restoring old metadata can overwrite a newer user's edit, so requires a new decision and fresh observation.

Private operation directories are mode 0700 at `~/.cueward/operations/files-tags/<id>/`; receipt/claim files are mode 0600. Before dispatch, the parent flushes the generated ID and `files tags receipt --operation-id` to stderr. Preserve it when cancelling. The worker uses the same parent-owned socket lifeline as mkdir/copy: parent SIGINT/SIGTERM/SIGKILL, EOF or transport failure stops the worker; a dead parent before startup prevents writes. Timeout stops the owned process group and records uncertain. Submitted kernel/provider metadata work is not undone. Receipt/backup data and lock files have no automatic expiry. These are process-interruption checkpoints, not power-loss transactions.

## Verification and remaining work

Existing regressions now exercise adding/removing existing tags, present-empty attributes, unselected colors/FinderInfo/xattrs, and observed external edits before submission, while retaining atomic-create, stale-version, conflict and interruption checks. A macOS 27.0.1 release CLI run on 2026-10-03 edited an owned existing colored tag array, preserving unselected colors, file bytes and unrelated xattrs. No personal tags or preferences were edited.

Legacy label editing, arbitrary older tag formats, global Finder preferences and real File Provider/TCC/read-only/unmount behavior remain unverified or unsupported. Other management operations are documented in [file mutations](files-mutations.md), [relocation](files-relocation.md) and [trash](files-trash-execution.md). Permanent deletion is excluded.

References: Apple's [tagNamesKey](https://developer.apple.com/documentation/foundation/urlresourcekey/tagnameskey) describes a read-write array of names; [fsetxattr](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fsetxattr.2.html) describes descriptor writes and presence guards. Signatures/constants are checked against the local Darwin SDK; the tag storage representation is based on owned-fixture evidence, not inferred from those API documents.
