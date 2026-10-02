# Authorized Finder tag-name edits

Check installed `files tags --help` before using these source instructions. Use read/add/remove only for the user's selected file/directory and requested names. Revisions do not authorize editing unrelated names, colors or file data.

```bash
cueward files tags read --root /absolute/directory --path report.txt
cueward files tags add --root /absolute/directory --path report.txt --expected-version '<file.version>' --expected-tags-version '<tags_version>' --tag Work --tag Review
cueward files tags read --root /absolute/directory --path report.txt
cueward files tags remove --root /absolute/directory --path report.txt --expected-version '<fresh file.version>' --expected-tags-version '<fresh tags_version>' --tag Review
cueward files tags receipt --operation-id '<announced operation ID>'
```

Read returns file identity/revision, ordered name/stored-color entries and tags_version. Both revisions are required for edits; reobserve after changing tags. Add only appends missing exact names; remove only removes exact matches. Preserve other names/order/colors. Names are case-sensitive without normalization/trimming; repeat --tag for multiple names. No set-all, color-edit, link-following, download or permanent-deletion alternative exists. Bounds are 1..256 requested names, 1024 UTF-8 bytes each, 8192 aggregate request bytes; names cannot contain NUL/CR/LF. Results are bounded to 256 entries/16384 name bytes. Worker timeout is 1..30000 ms, default 10000.

Available regular files, directories and packages can be selected without traversing package contents. Links, aliases/unknown alias state, placeholders/unknown availability and special files are rejected. Hardlinked files can be read but not edited because unselected names share their metadata. Stored color is observational, not a promise of Finder UI color or tag registration.

Only the validated bounded binary string-array tag representation is editable. Unknown shapes, inconsistent native names and legacy label-only representations are rejected, not treated as empty tags. Removing the last stored tag when a legacy FinderInfo label exists is rejected before writing; do not try to remove FinderInfo or unrelated attributes as a workaround. Use files metadata to read native names when tag storage is unsupported.

Add/remove return outer Ok even for a failed/uncertain receipt and exit nonzero unless status=completed and completion_verified=true. Interpret completed as verified observations (including a no-op), not current/future/cloud state. Not_started means known non-submission; incomplete means a successful attribute write followed by failure; uncertain means tags may have changed. Tags_receipt only loads saved evidence and does not retry, reconcile or restore.

Keep operation_id, receipt_path, status/stage/error, before/after and the pre-dispatch ID printed to stderr. Private 0700 operation directories contain 0600 receipts at ~/.cueward/operations/files-tags/<id>/receipt.json. Original_attribute_base64 backs up the prior tag attribute before writing; null is proof of absence only when before was observed. Evidence/backup data has no automatic expiry. On failure/cancellation, load the receipt and reobserve the actual object before deciding on a retry or manual recovery; do not automatically restore old tags over a newer edit or delete evidence.

Writes use the observed inode descriptor, not a re-looked-up pathname. A replacement inode cannot be tagged through a path race. Another actor can still move that selected inode outside the original path, and a submitted write can affect the moved object. Cueward editors share an inode lock; Finder/providers do not, and value revision checks are not atomic compare-and-swap. An external edit between the last check and setter can be overwritten or cause verification failure. Do not promise atomic isolation, unchanged future state, Finder UI refresh or provider synchronization.

Parent cancellation/EOF stops the worker through the mkdir/copy lifetime mechanism; timeout kills its process group. Submitted metadata/provider work is not rollback. Preserve the announced ID and inspect saved evidence after parent SIGINT/SIGTERM/SIGKILL. Real Finder UI, legacy label editing, older formats, TCC/read-only/unmount/provider and other volumes remain unverified or unsupported; local fixture success does not establish those capabilities.
