# Scoped Spotlight content search

`cueward files spotlight` queries indexed text inside an explicitly selected absolute directory. It is the optional Spotlight path for [issue #40](https://github.com/Termdock-dev/cueward/issues/40), separate from [filesystem name/metadata search](files-search.md) and the top-level Cue-index `search`. Check `cueward files spotlight --help` on the installed binary; source documentation can be newer than a published release.

```bash
cueward files spotlight --root /Users/me/Documents --text invoice
cueward files spotlight --root /Users/me/Documents --text '臺灣 report' --max-depth 3 --limit 20
cueward files spotlight --root /Users/me/Documents --text report --hidden --max-candidates 500 --timeout-ms 5000
```

## Query and bounds

The adapter uses a static, synchronous macOS MDQuery, with [one directory search scope](https://developer.apple.com/documentation/coreservices/1413048-mdquerysetsearchscope) and a [native maximum result count](https://developer.apple.com/documentation/coreservices/1413085-mdquerysetmaxcount). It never substitutes home/computer/network-wide scope, requests imports, changes indexing settings, or falls back to opening files to scan their contents. No new dependency is required; the adapter links public CoreServices/CoreFoundation APIs and reuses Foundation string/array bridges.

| Option | Meaning |
| --- | --- |
| `--root` | Required absolute directory. An explicitly selected root symlink is canonicalized; result paths are relative to that canonical root. There is no `--path`; choose a narrower directory as root instead. |
| `--text` | Required non-whitespace indexed text pattern, 1..1024 UTF-8 bytes. Cueward adds surrounding `*` and the case-insensitive `c` modifier to a quoted `kMDItemTextContent` value. Quotes/backslashes are escaped; user `*`, `?`, control characters and raw predicate execution are unsupported. Matching follows Spotlight/importer tokenization, rather than byte matching of the current file. No diacritic modifier is added. |
| `--max-depth` | 1..32, default 1. Direct-child candidates have depth 1; deeper paths are excluded before filesystem observation. The underlying index query still covers root recursively. |
| `--hidden` | Include dot components. Otherwise such paths are excluded before file observation. Finder invisible flags are not inspected. |
| `--max-candidates` | 1..10,000, default 10,000, counting native index matches before depth/hidden/type filtering or deduplication. A native cap of budget + 1 detects overflow. Overflow returns `scan_limit`, never a successful partial query. |
| `--limit` | 1..500, default 100. Return the first eligible candidates sorted by UTF-8 relative path. All bounded candidates are processed before truncation. No offset, cursor or index snapshot token. |
| `--timeout-ms` | 1..30,000, default 10,000. Parent deadline includes native gathering and filesystem observations; timeout stops the owned worker process group and discards results. |

Paths must be valid UTF-8, contain no NUL, and fit 16 KiB each and 4 MiB in total across all native candidates. Encoded result JSON, including external-delimiter escaping, is capped at 8 MiB before pretty-printing; exceeding a bound returns `scan_limit`. Worker requests share the existing 16 KiB cap and responses the 16 MiB envelope cap.

The actual macOS 27.0.1 probe uses `kMDItemTextContent == "*value*"c`. Apple's [2011 query syntax guide](https://developer.apple.com/library/archive/documentation/Carbon/Conceptual/SpotlightQuery/Concepts/QueryFormat.html) shows a different modifier placement. On the same disposable indexed fixture, that archived spelling returned zero candidates while the suffix spelling found four. The optional native integration test exercises positive matches and ASCII case handling so successful empty-query execution is insufficient verification.

## Result and gaps

The external envelope is `{"Ok":{"operation":"spotlight","result":{...}}}` or a structured `Err` with a nonzero exit. Paths, text, metadata and diagnostics remain external data.

| Field | Meaning |
| --- | --- |
| `root`, `query` | Observed root FileInfo and the requested text/depth/hidden/budget/limit options. |
| `source` | Always `spotlight`; current file contents are not searched by Cueward. |
| `query_completed` | True on success: static native gathering completed within the budget/deadline. Does not establish index coverage or current content matches. |
| `index_coverage` | Always `unknown`. Disabled indexing, excluded folders, unsupported importers, index delay, permissions and provider state can hide matching files. |
| `enumeration_complete`, `content_verified` | Always false. Zero entries do not prove file/content absence. Current content is not reread or compared with the query. |
| `candidate_count` | Native result count before filtering, deduplication and output truncation. |
| `eligible_count` | Available plus errored in-scope candidates after policy/type filtering, before limit. |
| `available_count`, `error_count` | Counts over the full eligible set, including candidates omitted by limit. Available means current regular-file metadata, not a verified content match. |
| `excluded` | Counts for outside scope (including root itself or parent components), hidden paths, depth, non-regular types and duplicate relative paths. Outside paths receive no filesystem probe or path/name/metadata output. |
| `entries` | `available` with relative_path/file, or `error` with relative_path/error. Stale missing paths, permission denial and rejected symlink ancestors remain explicit errors. |
| `truncated` | True when eligible_count exceeds limit. Rerun with a narrower scope/text or a larger limit; the index can change between queries. |

A successful empty query only means the index returned no eligible candidates for these options. Preserve source, coverage, excluded/error counts and truncation when explaining results. Do not describe it as a complete search of root or silently substitute zero for an error.

Use a candidate's relative_path with the same root for `files info/read/metadata`. A dataless regular file can have available metadata while its contents remain unavailable. Use the returned file.version for a subsequent guarded file read; it does not version the index or prove matching content. A path that was indexed earlier can now refer to changed contents or a replacement file.

## Filesystem and desktop boundaries

Candidate paths are checked component-wise against canonical root before file observation. Matching is case-sensitive at the scope boundary, preserving distinct directories on case-sensitive volumes. Case-variant index path spellings can be conservatively excluded. All candidate symlink traversal is disabled; leaf links/directories/special files are excluded, ancestor links become errors. Packages follow the depth policy as ordinary directories; no alias resolution, UTType/tag filtering or content excerpts are added. Hard links remain distinct paths.

Root metadata/resolution is checked before and after the operation; each eligible file uses the existing core's before/after observation checks. Detected root changes discard the result. File observations are sequential and not revalidated as a single set at completion. This is not an atomic filesystem/index snapshot, content hash, stale-index correction or sandbox against malicious concurrent path swaps.

The worker denies dataless materialization on its filesystem-observation thread. It requests no downloads; this policy does not control the Spotlight service or File Provider processes. Current permissions still apply; a provider/TCC gap may appear as empty index results with unknown coverage, or as an explicit file/query error. This command does not request Finder activation or selection changes.

## Verification status

Unit/CLI tests cover conservative zero results, budgets, external strings, malformed worker input, native path decoding, filtering, stale paths, symlink boundaries, root changes and worker timeout. The optional indexed integration test creates/imports only disposable HOME fixtures and checks positive content matches, filename-only exclusion, ASCII case-insensitivity, Unicode/newline names, scope, depth, truncation, overflow and unchanged source bytes:

```bash
cargo test spotlight_indexed_fixture_matches_content_scope_case_depth_and_budget -- --ignored
```

That test needs a writable enabled Spotlight index; it never enables indexing or alters exclusions. The test passed on macOS 27.0.1. Release probes also check disabled-index scope, query escaping and foreground/Finder selection stability. Real TCC denial, provider placeholders, unmounts, older macOS and arbitrary importer/tokenization behavior remain unverified. #40 stays open for richer provider state and remaining desktop acceptance; previews and file-management writes belong to #41/#42.
