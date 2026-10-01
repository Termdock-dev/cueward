# Scoped file reads

The initial `files list/info/read` implementation is in [PR #44](https://github.com/Termdock-dev/cueward/pull/44), pending merge as of 2026-10-01. Use this reference only if `cueward files --help` exposes these commands. Installing this skill does not install that implementation. Do not infer that the latest crates.io release contains main or an open PR.

## Choose the scope and range

```bash
cueward files list --root /absolute/directory --limit 100
cueward files list --root /absolute/directory --path Reports --hidden --sort modified --descending
cueward files info --root /absolute/directory --path Reports/report.txt
cueward files read --root /absolute/directory --path Reports/report.txt --max-bytes 65536
cueward files read --root /absolute/directory --path Reports/report.txt --start-line 20 --line-count 10
cueward files read --root /absolute/directory --path data.bin --encoding hex --offset 1024 --max-bytes 256
```

Always choose an explicit absolute root within the user's scope. Paths are relative to root without `..`; do not expand into home or another volume to work around a denial. List is one level, not recursive. Dot hidden names are optional; package directories and Finder alias files have no special handling yet.

Default symlink behavior is no traversal. Info can report a leaf link, including a broken/outside link, without following it. Explicit `--follow-links` allows an operation path to resolve only within canonical root; listing child links still describes those links. Root itself may be an explicitly selected directory link. Paths must be valid UTF-8.

## Page and resume

List's limit is 1..500. It observes at most 10,000 entries including hidden ones; scan_limit is not a partial successful sorted page. Later pages require the previous `version` with `--expected-version` and `next_offset` with `--offset`; keep sort, descending and hidden options. Child metadata/name changes, including hidden and unpaged entries, invalidate the version. On changed, restart from the first page.

Read's default max-bytes is 65,536, capped at 1 MiB of source bytes. Select utf8, utf16-le, utf16-be or hex explicitly; decoding never substitutes replacement characters. Text containing NUL is rejected as binary_data. Byte offsets start at 0 and must align with the encoding; next_offset identifies the next complete character/byte after bounded truncation. An incomplete character at file EOF is decode_error, not successful text.

Line mode is UTF-8 only, starts at line 1, cannot combine with --offset, and scans at most 8 MiB before the first requested line. line-count is 1..10,000 (default 100); max-bytes still applies. LF separates lines and CRLF stays intact. `eof` means file EOF, not merely completion of the requested line count. `truncated` means more of the file remains. A partial line's next_line points to that same line; use next_offset in byte mode for exact resume without repeating text. Bind a subsequent read to `file.version` with --expected-version when needed.

## Interpret availability and failure

The external JSON contains `Ok` with operation/result, or `Err` with code/message and a nonzero exit. Decode JSON escapes before comparing contents: literal `<` is escaped to protect the external wrapper without changing decoded strings.

Metadata includes requested/resolved paths, kind, identity/revision, size, UTC timestamps, mode, readonly, data_state and link_target. Directory size is not a recursive total. readonly reflects Unix write bits, not TCC/readability. Identity and version are metadata observations, not content hashes or an atomic filesystem snapshot.

Dataless placeholders are distinct from empty files. The macOS worker denies dataless materialization before filesystem operations; it has no download action. not_dataless does not prove cloud/network/provider availability. Preserve unavailable, permission_denied, not_found, scan_limit, decode_error, changed and timeout as distinct failures; do not call them successful zero bytes/results.

The worker deadline defaults to 10 seconds, configurable with --timeout-ms 1..30000. The operation does not edit the source file or request Finder/app activation. On change/error/timeout, reobserve before resuming; it discards detected changed-file results, but is not a sandbox against malicious concurrent path swaps.

Search, Spotlight, UTType/Finder tags, package/alias enrichment, Finder selection/reveal and provider downloads are not part of this slice. Use app open only for an explicitly selected recipient; its sent_unverified result does not prove content was received. PDF/image preview and file-management actions are separate work.
