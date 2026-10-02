# Observed Finder tag names and guarded edits

`files tags read/add/remove/receipt` is a second slice of [#42](https://github.com/Termdock-dev/cueward/issues/42). Use installed command help to check availability. Edits require the user's requested effect and selected existing file/directory; revision tokens are observations, not authorization.

```bash
cueward files tags read --root /Users/me/Documents --path report.txt
cueward files tags add --root /Users/me/Documents --path report.txt --expected-version '<file.version>' --expected-tags-version '<tags_version>' --tag Work --tag Review
cueward files tags read --root /Users/me/Documents --path report.txt
cueward files tags remove --root /Users/me/Documents --path report.txt --expected-version '<fresh file.version>' --expected-tags-version '<fresh tags_version>' --tag Review
cueward files tags receipt --operation-id '<announced operation ID>'
```

## Selection and supported representation

All commands require one absolute root and a relative existing path. Read defaults to `.`; edits require explicit `--path`, `--expected-version`, `--expected-tags-version` and at least one `--tag`. Each command has a 1..30000 ms worker deadline, default 10000. Symlinks, Finder aliases, unknown alias state, dataless/unknown availability and special files are rejected. No link following, download, replacement tag-set, color editing or automatic retry option is provided. Ordinary directories and package directories can be selected; package contents are not traversed or read. Hardlinked regular files can be read but edits are rejected because metadata changes would also affect unselected names.

Add appends missing exact names once; an already-present name retains its order/color. Remove removes all exact matches, preserving the order/color of unselected entries. Matching is case-sensitive without normalization or whitespace trimming. Names must be nonempty, contain no NUL/CR/LF, and fit 1024 UTF-8 bytes each, 256 requested names and 8192 aggregate request-name bytes. The resulting stored list has at most 256 entries and 16384 aggregate name bytes. Duplicate requested names do not create duplicate tags. Adding existing names or removing absent names is a verified no-op, not an attribute rewrite.

The adapter supports the bounded binary-plist string-array representation observed on macOS 27.0.1 in `com.apple.metadata:_kMDItemUserTags`. Entries may be unsuffixed names or names with a stored newline plus color digit 0..7. Existing names/color suffixes are retained; new names use stored color 0. Snapshot `color=null` means no suffix was stored, not an unknown name or a queried Finder preference. This is not a guarantee of current Finder UI color, sidebar registration or global tag preferences. Apple documents tag names through NSURL, but does not establish this on-disk representation as a universal API contract; unsupported shapes are rejected instead of reconstructed or silently erased.

A missing tag attribute is established by descriptor `ENOATTR`, not by assuming a missing Foundation resource is an empty list. Its revision differs from a stored empty array. Native Foundation names must agree with decoded names; missing resource values are accepted only when the descriptor establishes a known empty set. Malformed/oversized attributes, unknown color suffixes, inconsistent native names and unsupported filesystems remain errors.

Legacy FinderInfo labels are not edited. A label-only file whose native names differ from the stored tag representation is rejected; use `files metadata` to observe its native names. Removing the last stored tag while a legacy label exists is rejected before mutation, because otherwise that label can reappear as a native tag. Other FinderInfo bytes and unrelated extended attributes are never intentionally rewritten. This slice is not a full Finder label synchronization implementation.

## Write boundary and verification

The adapter opens an available target through no-follow descriptors and checks its identity/version against the selected path. It reads bounded attributes without opening the data stream for copying. Generic core code produces the name edit; macOS submits one descriptor `fsetxattr`, never a path-based setter that could tag a replacement inode. Cueward serializes tag editors of the same inode with a nonblocking lock. The exact file revision and raw tag-attribute revision are rechecked immediately before submission; CREATE/REPLACE additionally protects whether the attribute exists.

After submission, the worker verifies stored names/colors, a fresh Foundation name view, target/root identity, target size/mtime/mode and final descriptor/path revisions. Detected relocation, replacement or read-back failure prevents `completed`. Content is not hashed because tag operations do not read or write payload bytes; preserving content bytes was checked separately with owned fixtures. Receipts do not prove current/future state, cloud synchronization or Finder refresh.

This is an observed-inode metadata edit, not namespace confinement. External actors can move the selected inode after checks; a submitted descriptor write can still affect that same inode at its moved location, but cannot switch to a replacement inode through a pathname lookup. External Finder/provider editors do not honor the Cueward lock. There is no kernel compare-and-swap for attribute values: an external edit between the final revision check and the setter can be overwritten or can cause post-verification failure. Version checks detect observed conflicts, not every concurrent race. Do not use this slice for workflows requiring atomic isolation from other metadata editors.

## Receipts, interruption and retention

External JSON is `Ok` with operation `tags_read`, `tags_edit` or `tags_receipt`, or structured `Err`. Paths, names and diagnostics are escaped within the shared `<external>` envelope. Add/remove exit nonzero unless status is `completed` and `completion_verified=true`; outer Ok alone does not establish success. Receipt exits zero when evidence was read, regardless of its status.

- `completed`: requested names/colors and final observations verified, including no-op edits.
- `not_started`: a known rejection prevented tag submission; an existing tag set is not overwritten by this operation.
- `incomplete`: a native tag write succeeded but later observation/checkpoint verification failed; changed tags remain.
- `uncertain`: delivery or completion cannot be established. Tags may have changed even if the last checkpoint predates submission.

Receipt fields include operation_id, request, receipt_path, status/stage/error, mutation_attempted, changed_by_operation, completion_verified, before/after snapshots and original_attribute_base64. The original attribute is saved before submission for authorized manual recovery; null with a before snapshot records proven absence, while null without a before snapshot establishes nothing. There is no automatic restore or deletion. Restoring old metadata can overwrite a newer user's edit, so requires a new decision and fresh observation.

Private operation directories are mode 0700 at `~/.cueward/operations/files-tags/<id>/`; receipt/claim files are mode 0600. Before dispatch, the parent flushes the generated ID and `files tags receipt --operation-id` to stderr. Preserve it when cancelling. The worker uses the same parent-owned socket lifeline as mkdir/copy: parent SIGINT/SIGTERM/SIGKILL, EOF or transport failure stops the worker; a dead parent before startup prevents writes. Timeout stops the owned process group and records uncertain. Submitted kernel/provider metadata work is not undone. Receipt/backup data and lock files have no automatic expiry. These are process-interruption checkpoints, not power-loss transactions.

## Verification and remaining work

Owned disposable tests cover name/color/order preservation, untagged versus empty attributes, no-op edits, file/tag stale revisions, observed concurrent tag changes and pathname replacement before submission, checkpoint failure before/after writes, regular files/directories/packages, hardlink edit rejection, scope/link rejection, malformed attributes, private receipts and rejected replay. CLI tests cover parsing, external strings, receipt reload, exit semantics and delayed-start parent interruption. Native fixtures also compare unchanged inode/content/mtime/permissions, unrelated xattrs and FinderInfo bytes. No personal tags or Finder preferences are edited.

Real Finder UI refresh, legacy label editing, arbitrary older macOS storage formats, network/external volumes, File Provider/TCC/read-only/unmount cases and global tag preferences remain unverified or unsupported as described above. Move/rename/duplicate, recursive copy and trash/recovery remain subsequent #42 work; permanent deletion is not included.

References: Apple's [tagNamesKey](https://developer.apple.com/documentation/foundation/urlresourcekey/tagnameskey) describes a read-write array of names; [fsetxattr](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fsetxattr.2.html) describes descriptor writes and presence guards. Signatures/constants are checked against the local Darwin SDK; the tag storage representation is based on owned-fixture evidence, not inferred from those API documents.
