# Scoped file search, metadata and reads

The initial `files list/info/read` implementation was merged in [PR #44](https://github.com/Termdock-dev/cueward/pull/44). Filesystem search was merged in [PR #46](https://github.com/Termdock-dev/cueward/pull/46). Resource metadata was merged in [PR #47](https://github.com/Termdock-dev/cueward/pull/47). Finder context/reveal was merged in [PR #48](https://github.com/Termdock-dev/cueward/pull/48). This reference also describes Spotlight (merged in [PR #49](https://github.com/Termdock-dev/cueward/pull/49)), cloud (merged in [PR #50](https://github.com/Termdock-dev/cueward/pull/50)) and preview (merged in [PR #51](https://github.com/Termdock-dev/cueward/pull/51)). Check `cueward files --help`, `cueward files search --help`, `cueward files metadata --help`, `cueward files finder --help`, `cueward files spotlight --help`, `cueward files cloud --help` and `cueward files preview --help` before relying on these capabilities. Installing a skill does not install a binary; documentation can describe a newer source revision than main or the latest published release.

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

## Find indexed text with Spotlight

```bash
cueward files spotlight --root /absolute/directory --text invoice
cueward files spotlight --root /absolute/directory --text '臺灣 report' --max-depth 3 --limit 20
```

Use this separate command only when installed and the task asks for indexed content candidates. Root is required; no --path, --follow-links, offset or snapshot cursor is supported. Text is a case-insensitive Spotlight substring pattern, not a raw predicate or current-byte scan. It must contain non-whitespace and fit 1024 UTF-8 bytes, excluding controls and user * or ?. Quotes/backslashes are escaped. Tokenization/importer behavior can differ from literal file bytes; Cueward returns no excerpts and does not verify matching content.

Depth is 1..32 (default 1), hidden includes dot components, max-candidates is 1..10,000 (default 10,000), limit is 1..500 (default 100), and timeout-ms is 1..30,000 (default 10,000). The index query covers root recursively; depth/hidden filters apply before filesystem observation, after the native budget. Oversized queries/candidate sets fail instead of returning partial success. Each candidate path is bounded to 16 KiB, aggregate paths to 4 MiB, and escaped compact result JSON to 8 MiB.

The response operation/source is spotlight. query_completed=true means the bounded native query finished. index_coverage is always unknown; enumeration_complete and content_verified are always false. Disabled/excluded indexes, unsupported importers, stale data, permission/provider gaps and filtering can hide matches. Zero results never establish file/content absence. Preserve candidate_count, excluded counts and errors when explaining a result.

Entries sort by relative_path and have available (current regular-file metadata) or error (relative_path/code/message) status. Missing candidates and ancestor symlinks remain errors; no candidate symlink traversal occurs. Outside scope receives only an exclusion count. Dataless files can retain available metadata without readable contents. eligible_count, available_count and error_count cover the full filtered set before limit; truncated=true means returned entries omit some of that set, possibly including errors. Narrow the scope/text or increase limit; repeated queries are not stable pages.

Use relative_path with the same root for info/read/metadata and file.version for a guarded subsequent read. The version does not bind index state or prove the current content matched. Root changes discard results, while file checks are sequential observations rather than one atomic snapshot. The command neither enables indexing nor requests imports/downloads or Finder activation; worker no-materialization does not control Spotlight/provider processes. Actual provider/TCC/unmount acceptance remains incomplete.

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

## Inspect iCloud state or explicitly request a download

```bash
cueward files cloud status --root /absolute/directory --path Reports/report.pdf
cueward files cloud download --root /absolute/directory --path Reports/report.pdf --expected-version '<fresh file version>' --max-bytes 33554432
```

Check installed cloud capabilities first. Status reads metadata/resource keys without opening contents or requesting downloads. It returns cloud_status with file, resources, provider_coverage=icloud_keys_other_providers_unknown and download_requested_by_operation=false. The keys apply to iCloud, not uniform third-party provider state. An unavailable is_ubiquitous is unknown, not false or local-only. Available/false membership makes dependent fields not_applicable; unavailable/error membership leaves dependent fields unavailable. Leaf links and special files have not_applicable fields; explicit following must stay within root. Leaf placeholders can be observed, but dataless roots/ancestors remain unavailable.

Inspect each resource's status before using value. downloading_status.value.state is not_downloaded, downloaded, current or other (with preserved native value). is_downloading reports an active download; download_requested alone can reflect a past request. is_uploaded/is_uploading, has_unresolved_conflicts, downloading_error and uploading_error retain independent availability/errors. Missing error values do not prove absence of errors. Sequential resource observations are not an atomic snapshot, and file.version does not version provider state.

Download is an explicit user-requested effect, never a silent fallback for failed reads/searches/previews. Select one regular file from fresh status and require its file.version with --expected-version. Membership must be available/true. Directories, unfollowed links, special files and unknown/non-iCloud membership are rejected. max-bytes (default 33554432, range 1..268435456) bounds reported size before submission, not provider transfer bytes or final size; there is no cancellation or directory download.

The cloud_download receipt includes operation_id, before, max_bytes, status, download_requested_by_operation, completion_verified=false and post_check=Ok(CloudStatus) or Err(code/message). sent_unverified means a new native request was accepted; already_current or already_requested means no new request was made. None proves completed readable/current bytes. A failed post_check can coexist with an already submitted request; preserve both facts. Normal download metadata changes are allowed after submission, while scoped item identity/path/root are checked.

timeout-ms (default 10000, range 1..30000) bounds submission/observation, not the asynchronous provider transfer. The operation lock is released after post-check. A timeout or worker failure can hide an accepted request and cannot cancel it. There is no automatic retry. Observe fresh cloud status before retrying, and independently read the requested versioned content before claiming it is readable. Real iCloud transfer/placeholder/TCC/unmount and generic provider acceptance remain unverified.

## Read Finder context or reveal a chosen item

```bash
cueward files finder context --root /absolute/directory --max-items 100
cueward files finder reveal --root /absolute/directory --path Reports/report.txt --expected-version '<previous file version>'
```

Context reads Finder's front window target and current selection without requesting activation. Root is required; only scoped items receive file information. Window order belongs to Finder even when another app is foreground. The response operation is finder_context; root is FileInfo, front_window is id/location or null, and selection is an array in Finder order or null when Finder is not running. A null finder_pid and false selection_complete mean no queryable process was observed. An available empty selection is [].

Location/selection entries are available (relative_path/file), outside_scope (no item path/name/file information), or error (code/message). Native virtual/remote/non-UTF-8 URLs remain errors. Leaf links are described without following; path traversal through links is rejected. Do not turn excluded/error entries into empty selection. max-items accepts 1..500, defaults to 100; exceeding it returns scan_limit without a partial success. selection_complete covers the query's full array, not metadata availability or an atomic UI snapshot. Root/file versions do not bind Finder UI state.

Context needs Apple Events permission to Finder; a permission_denied failure is not an empty selection. Inspect the invoking host's Automation access. If a location lies outside the chosen root, obtain a permitted scope instead of expanding to home or another volume automatically.

Reveal is an explicit action that can launch/activate Finder, open a window and change selection. Use it only for that requested effect. It requires root and an explicit relative path. An unfollowed symlink/special file or dataless item is rejected; follow-links may resolve only within root. The root bounds the submitted item, not Finder's displayed parent or other windows. Worker no-materialization does not control Finder's own preview/provider behavior.

The response operation is finder_reveal and status is sent_unverified. activation_requested and selection_change_requested are true. foreground contains optional before/after PIDs and foreground_changed; an unchanged immediate PID does not rule out later activation. post_check is Ok(FileInfo) or Err(code/message), checking file/root state after submission rather than UI delivery. An outer Ok with a failed post_check is still a submitted action. Verify the intended selection with fresh context/independent observation; never claim completion from status or post_check alone.

Reveal holds a global operation lock and the running Finder's shared input lock during submission/post-check, then releases them. A busy lock or detected process change blocks submission; asynchronous UI completion is outside that lock period. A timeout/invalid worker response may hide an already delivered reveal. Inspect before retrying; the command neither retries nor restores foreground automatically. Both commands use timeout-ms 1..30000, default 10000. Actual reveal UI delivery and provider/TCC/unmount acceptance remain unverified in this revision.

## Page and resume

List's limit is 1..500. It observes at most 10,000 entries including hidden ones; scan_limit is not a partial successful sorted page. Later pages require the previous `version` with `--expected-version` and `next_offset` with `--offset`; keep sort, descending and hidden options. Child metadata/name changes, including hidden and unpaged entries, invalidate the version. On changed, restart from the first page.

Read's default max-bytes is 65,536, capped at 1 MiB of source bytes. Select utf8, utf16-le, utf16-be or hex explicitly; decoding never substitutes replacement characters. Text containing NUL is rejected as binary_data. Byte offsets start at 0 and must align with the encoding; next_offset identifies the next complete character/byte after bounded truncation. An incomplete character at file EOF is decode_error, not successful text.

Line mode is UTF-8 only, starts at line 1, cannot combine with --offset, and scans at most 8 MiB before the first requested line. line-count is 1..10,000 (default 100); max-bytes still applies. LF separates lines and CRLF stays intact. `eof` means file EOF, not merely completion of the requested line count. `truncated` means more of the file remains. A partial line's next_line points to that same line; use next_offset in byte mode for exact resume without repeating text. Bind a subsequent read to `file.version` with --expected-version when needed.

## Interpret availability and failure

The external JSON contains `Ok` with operation/result, or `Err` with code/message and a nonzero exit. Decode JSON escapes before comparing contents: literal `<` is escaped to protect the external wrapper without changing decoded strings.

Metadata includes requested/resolved paths, kind, identity/revision, size, UTC timestamps, mode, readonly, data_state and link_target. Directory size is not a recursive total. readonly reflects Unix write bits, not TCC/readability. Identity and version are metadata observations, not content hashes or an atomic filesystem snapshot.

Dataless placeholders are distinct from empty files. The macOS worker denies dataless materialization before filesystem operations; read-only commands do not request downloads. Use the separate explicit cloud download only for that requested effect. not_dataless does not prove cloud/network/provider availability. Preserve unavailable, permission_denied, not_found, scan_limit, decode_error, changed and timeout as distinct failures; do not call them successful zero bytes/results.

The worker deadline defaults to 10 seconds, configurable with --timeout-ms 1..30000. List/info/read/search/metadata/spotlight do not edit the source file or request Finder/app activation. On change/error/timeout, reobserve before resuming; it discards detected changed-file results, but is not a sandbox against malicious concurrent path swaps.

UTType/tag search filters, package pruning, alias resolution and generic third-party provider downloads are not part of this slice. Use app open only for an explicitly selected recipient; its sent_unverified result does not prove content was received. PDF/image/Quick Look previews have a separate command below; authorized mkdir/single-file copy and saved receipts use [file-mutations.md](file-mutations.md); other file-management actions remain subsequent work.


## Preview PDF pages, images or a Quick Look thumbnail

```bash
cueward files preview pdf --root /absolute/directory --path report.pdf --start-page 2 --page-count 3
cueward files preview pdf --root /absolute/directory --path scanned.pdf --ocr --no-render
cueward files preview image --root /absolute/directory --path photo.png --ocr
cueward files preview thumbnail --root /absolute/directory --path report.docx
```

Use installed preview capabilities for one explicitly selected regular file. Root and relative path are required. Default no-follow and in-root explicit follow-links apply; directories/packages are unsupported. Thumbnail additionally rejects Finder aliases without resolving their targets and requires available/false native alias state. Dataless/unknown-availability data is unavailable, never empty; no preview requests a download. Expected-version binds a prior source metadata revision.

PDF rendering includes native page rotation and displayed annotations. PDF uses native text first and applies explicit --ocr only to absent/whitespace-only native page text. Image information/preview/OCR cover only the first oriented frame. Thumbnail is provider-dependent visual content only, with no text/point/page extraction; icons are rejected. Use files read for bounded plain text. No Office/iWork full-text parser is provided, and a thumbnail never establishes whole-document readability.

PDF start-page is 1..1000000 (default 1), page-count is 1..20 (default 10). Max-input-bytes is 1..268435456 (default 67108864); max-text-bytes is an aggregate UTF-8 budget of 1..1048576 (default 65536); max-dimension is 1..2048 (default 1024); max-preview-bytes is an aggregate PNG budget of 1..33554432 (default 8388608). Timeout-ms 1..30000 (default 10000) includes snapshot, Swift startup and processing. No-render suppresses persisted PDF/image PNGs while explicit OCR can still render a bounded transient image. These bound accepted content/output, not native/provider peak allocation or expansion cost.

The preview result includes source file identity/version/timestamps, source_sha256, kind, optional PDF count/encryption and image metadata, pages, selection_truncated, text_truncated and optional cache_directory. Each page has independent text/preview resource statuses, text_source, optional confidence and text_truncated. Available empty text, unavailable native text, an unrequested resource and an error are different. An outer Ok can contain resource failures; preserve them, page range and truncation instead of claiming complete extraction. Thumbnail always has selection_truncated=true.

Text_source is native_pdf, vision_ocr or not_requested. Native text has no confidence. Vision confidence is the mean of top-candidate line scores, not a correctness probability; low-confidence text is retained, and empty recognition has null confidence. Source SHA-256 hashes the guarded private input snapshot. Before/after file/root checks reject detected changes, but do not create an atomic filesystem snapshot or defend against malicious swaps.

Available PNGs report absolute path, actual dimensions/bytes and SHA-256. Private mode-0700 directories under ~/.cueward/cache/file-previews/preview-* retain only successful PNGs; input/helper files are removed, failures/timeouts clean the owned directory, and no previous cache is reused. View the actual reported artifact to confirm visual content. Keep artifacts only as needed; remove only the exact owned cache after consuming it. The command never edits/removes the source.

Preview uses the macOS Swift/Command Line Tools setup; an unavailable helper/toolchain is distinct from a missing source file. Locked PDFs are encrypted errors, recognized undecodable inputs can be corrupt_data, and unsupported formats remain unsupported_type. PDF copy restrictions remain permission_denied. Page OCR/render/thumbnail errors preserve their field diagnostics. PDFKit/ImageIO recognition is not exhaustive validation of every damaged format. Timeout is not an empty document. Worker/helper materialization guards and owned-group termination do not control separate Quick Look provider services.

Native PDF/image/rotation/scanned-text fixtures and the opt-in PDF Quick Look probe passed on macOS 27.0.1. A release probe also produced and decoded content thumbnails for PDF, PNG, plain text and a minimal DOCX; the DOCX thumbnail was visually inspected. Other Office/iWork and Quick Look formats depend on installed providers; use actual field status and artifact evidence rather than assuming all variants work. Real TCC/provider/unmount and older macOS acceptance remain incomplete.

A separate live probe saw an unattributed foreground/Finder change; follow-up snapshots after every preview call remained identical. Do not infer continuous input isolation or all-provider behavior from those discrete checks.
