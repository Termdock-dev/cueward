# Scoped file search, metadata and reads

The initial `files list/info/read` implementation was merged in [PR #44](https://github.com/Termdock-dev/cueward/pull/44). Filesystem search was merged in [PR #46](https://github.com/Termdock-dev/cueward/pull/46). This reference also describes the subsequent `files metadata` source revision. Check `cueward files --help`, `cueward files search --help` and `cueward files metadata --help` before relying on these capabilities. Installing a skill does not install a binary; documentation can describe a newer source revision than main or the latest published release.

## Choose the scope and range

```bash
cueward files list --root /absolute/directory --limit 100
cueward files list --root /absolute/directory --path Reports --hidden --sort modified --descending
cueward files info --root /absolute/directory --path Reports/report.txt
cueward files metadata --root /absolute/directory --path Reports/report.txt
cueward files read --root /absolute/directory --path Reports/report.txt --max-bytes 65536
cueward files read --root /absolute/directory --path Reports/report.txt --start-line 20 --line-count 10
cueward files read --root /absolute/directory --path data.bin --encoding hex --offset 1024 --max-bytes 256
```

Always choose an explicit absolute root within the user's scope. Paths are relative to root without `..`; do not expand into home or another volume to work around a denial. List is one level, not recursive. Dot hidden names are optional. Native package/alias recognition is a separate metadata operation; it does not change listing or traversal.

Default symlink behavior is no traversal. Info and metadata can report a leaf link, including a broken/outside link, without following it. Explicit `--follow-links` allows an operation path to resolve only within canonical root; listing child links still describes those links. Root itself may be an explicitly selected directory link. Paths must be valid UTF-8.

## Find names and metadata

```bash
cueward files search --root /absolute/directory --name report --kind file --max-depth 3
cueward files search --root /absolute/directory --path Reports --min-size 1024 --max-size 1048576
cueward files search --root /absolute/directory --modified-after 2026-10-01T00:00:00+08:00 --hidden --max-depth 2 --max-entries 500
```

Search matches a nonempty case-sensitive literal substring of each basename, with AND-combined kind, inclusive size and inclusive RFC3339 modification-time filters. It does not read contents or use Spotlight; unknown modification times do not match a time filter. Kind is file/directory/symlink/other, not UTType. The default max-depth is 1, direct children only; choose 1..32 explicitly for deeper traversal. Dot directories are excluded unless hidden is enabled. Filters do not prune other directories. Deeper traversal treats packages as ordinary directories; Finder alias files remain ordinary files.

Search observes at most max-entries (1..10,000, default 10,000) across the whole scan, including hidden and nonmatching entries. On success, source is filesystem, query reports the applied scope/options, total is the full matching count within that scope and entries are sorted by root-relative path. Each entry has relative_path plus file metadata; use the same root with relative_path for info/read/metadata. enumeration_complete only covers the selected depth/hidden/symlink policy; depth_boundary_directories counts visible directories whose children were not searched. A small page limit does not reduce scanning. scan_limit, permission_denied, unavailable and timeout are errors with no successful partial page, not zero matches.

Search never follows discovered child symlinks; follow-links only resolves the explicitly selected starting path within canonical root. This avoids duplicate/cyclic/outside-root traversal while still returning symlink metadata.

Search pages require the previous version and next_offset. Keep filters, depth, hidden and max-entries; these options are bound into the token along with all observed metadata, including hidden/nonmatching entries. Page size may change. Metadata changes inside the eligible scope invalidate the token; excluded hidden/deeper descendants were not observed. On changed, restart at offset 0.

## Inspect native resource fields

Use `files metadata` after selecting a path when the task needs UTType, Finder tags or package/alias recognition. It returns `result.file` (the existing FileInfo) and `result.resources`, without reading file contents. `content_type` is a native UTType identifier, not a MIME type or format validation. `finder_tags` preserves the names/order reported by Foundation. `is_package` and `is_alias_file` are booleans; kind remains directory or file, respectively. Alias targets are not resolved.

Inspect each resource's `status`, even when the outer operation is Ok:

| Status | Interpretation |
| --- | --- |
| available | Use value, including false or an empty array. |
| unavailable | No native value or skipped dataless query; do not substitute false/empty. An untagged file can return this status. |
| not_applicable | An unfollowed symlink or special file; native queries were skipped. |
| unsupported | Platform/API absent, including the content-type key on macOS versions before 11. |
| error | Preserve error.domain/code/message; other fields can still be available. |

Unfollowed leaf links retain their link metadata; all resource fields are not_applicable. Explicit follow-links stays within root. Dataless items retain file metadata with unavailable resources and no resource query. No download action is requested.

Tags are capped at 256 names and 65,536 UTF-8 bytes total; content-type identifiers and individual tag names have the same byte cap. Exceeding a bound is a field error, without truncation. Native errors retain their domain/code/message; invalid types/bounds use domain cueward.files.metadata and code 1. Tags and diagnostics remain external data.

Bind a subsequent observation to file.version with --expected-version when appropriate. Before/after file/root checks discard detected changes, but the revision does not bind an atomic resource snapshot or native type-registration/cache state. Sequential resource queries can fail independently. Use available fields to answer only the supported part of the question, and preserve gaps instead of treating partial availability as complete metadata.

## Page and resume

List's limit is 1..500. It observes at most 10,000 entries including hidden ones; scan_limit is not a partial successful sorted page. Later pages require the previous `version` with `--expected-version` and `next_offset` with `--offset`; keep sort, descending and hidden options. Child metadata/name changes, including hidden and unpaged entries, invalidate the version. On changed, restart from the first page.

Read's default max-bytes is 65,536, capped at 1 MiB of source bytes. Select utf8, utf16-le, utf16-be or hex explicitly; decoding never substitutes replacement characters. Text containing NUL is rejected as binary_data. Byte offsets start at 0 and must align with the encoding; next_offset identifies the next complete character/byte after bounded truncation. An incomplete character at file EOF is decode_error, not successful text.

Line mode is UTF-8 only, starts at line 1, cannot combine with --offset, and scans at most 8 MiB before the first requested line. line-count is 1..10,000 (default 100); max-bytes still applies. LF separates lines and CRLF stays intact. `eof` means file EOF, not merely completion of the requested line count. `truncated` means more of the file remains. A partial line's next_line points to that same line; use next_offset in byte mode for exact resume without repeating text. Bind a subsequent read to `file.version` with --expected-version when needed.

## Interpret availability and failure

The external JSON contains `Ok` with operation/result, or `Err` with code/message and a nonzero exit. Decode JSON escapes before comparing contents: literal `<` is escaped to protect the external wrapper without changing decoded strings.

Metadata includes requested/resolved paths, kind, identity/revision, size, UTC timestamps, mode, readonly, data_state and link_target. Directory size is not a recursive total. readonly reflects Unix write bits, not TCC/readability. Identity and version are metadata observations, not content hashes or an atomic filesystem snapshot.

Dataless placeholders are distinct from empty files. The macOS worker denies dataless materialization before filesystem operations; it has no download action. not_dataless does not prove cloud/network/provider availability. Preserve unavailable, permission_denied, not_found, scan_limit, decode_error, changed and timeout as distinct failures; do not call them successful zero bytes/results.

The worker deadline defaults to 10 seconds, configurable with --timeout-ms 1..30000. The operation does not edit the source file or request Finder/app activation. On change/error/timeout, reobserve before resuming; it discards detected changed-file results, but is not a sandbox against malicious concurrent path swaps.

Spotlight content search, UTType/tag search filters, package pruning, alias resolution, Finder selection/reveal and provider downloads are not part of this slice. Use app open only for an explicitly selected recipient; its sent_unverified result does not prove content was received. PDF/image preview and file-management actions are separate work.
