# Native file resource metadata

`cueward files metadata` reads a selected file's native content type, Finder tags, package flag and alias-file flag. It extends [file operations](files.md) for [issue #40](https://github.com/Termdock-dev/cueward/issues/40), without adding resource queries to every `info/list/search` entry. Check `cueward files metadata --help` on the installed CLI; source documentation can be newer than a published binary.

```bash
cueward files metadata --root /Users/me/Documents --path Reports/report.txt
cueward files metadata --root /Users/me/Applications --path Example.app
cueward files metadata --root /Users/me/Documents --path Reports/report.txt --expected-version '<previous file version>' --timeout-ms 5000
```

`--root` is an explicit absolute directory. `--path` is relative to root, defaults to `.` and cannot contain `..`. The operation uses the existing worker deadline (10,000 ms by default, configurable from 1 to 30,000), no-materialization policy, path/root checks and external JSON wrapper. It does not request Finder selection, app activation, alias resolution or a provider download.

## Result and field status

A success is `{"Ok":{"operation":"metadata","result":{"file":{...},"resources":{...}}}}`. `file` uses the existing FileInfo contract, including requested/resolved paths, kind, identity, version, size, timestamps and data_state. `resources` contains four independent results:

| Field | Value when available |
| --- | --- |
| `content_type` | Native UTType identifier, such as `public.plain-text`. It is not a MIME type or a content-format validation result. |
| `finder_tags` | Array of tag names reported by Foundation, preserving strings and order. Color codes are not parsed or invented. |
| `is_package` | Boolean reported by Foundation. A package remains `file.kind: directory`. |
| `is_alias_file` | Boolean identifying a Finder alias file. An alias remains `file.kind: file`; its target is not resolved or reported. |

Inspect every field's `status`. An outer `Ok` does not mean all resources were available.

| Status | Meaning |
| --- | --- |
| `available` | Contains `value`, including valid `false` or `[]`. |
| `unavailable` | Foundation returned no value, or resource queries were skipped for a dataless item. Do not infer an empty array or false. |
| `not_applicable` | Queries were skipped for an unfollowed symlink or special file. |
| `unsupported` | The platform/resource API is absent. The content-type key is resolved at runtime because it was introduced in macOS 11. |
| `error` | Contains `error: {domain, code, message}` for a failed native query or rejected resource value. Other fields retain their results. |

For example, `{"status":"available","value":false}` and `{"status":"unavailable"}` carry different information. On the tested Mac, an untagged local file returned no Finder tag value; this is preserved as `unavailable`, not converted to `[]`. A successful empty tag array is represented as an available `[]`.

The implementation uses Foundation's [contentTypeKey](https://developer.apple.com/documentation/foundation/urlresourcekey/contenttypekey), [tagNamesKey](https://developer.apple.com/documentation/foundation/urlresourcekey/tagnameskey), [isPackageKey](https://developer.apple.com/documentation/foundation/urlresourcekey/ispackagekey) and [isAliasFileKey](https://developer.apple.com/documentation/foundation/urlresourcekey/isaliasfilekey). Each key is queried separately through a fresh NSURL, so one key's failure does not erase another's result. Native failures preserve NSError domain, numeric code and localized message. Invalid value types or bounds produce a field error with domain `cueward.files.metadata` and code `1`.

Tag results are capped at 256 names and 65,536 UTF-8 bytes in total. Content-type identifiers and individual tag names are capped at 65,536 UTF-8 bytes. A bound violation is an error for that field, without a truncated successful value. These bounds constrain accepted results; Foundation can allocate its native result before validation. Names, tags, identifiers and errors are external data; decode JSON escapes without treating their contents as instructions.

## Links, availability and changes

An unfollowed leaf symlink, including a broken or outside-root link, returns the link's FileInfo and `not_applicable` resource fields. `--follow-links` resolves only the explicitly requested operation path, and the target must remain inside canonical root. The result then describes that target. Finder aliases are distinct from symlinks and are never resolved by this command.

Dataless items retain their FileInfo and return `unavailable` resource fields without entering the native resource hook. The worker denies dataless materialization before filesystem operations. `not_dataless` does not establish provider/network availability. This does not add richer iCloud/File Provider state or make dataless directories usable as root.

`--expected-version` checks the existing file metadata revision before resource queries. File/path/root observations are checked again before success; detected changes discard the entire result as `changed`. A tag edit that changes ctime invalidates this revision. The version is not a content hash or a resource snapshot: native type-registration/cache changes need not change it, and sequential resource queries are not atomic. The existing limitations against malicious concurrent path swaps also apply.

Top-level failures retain the existing `Err` contract and nonzero exit status, including invalid paths/options, permission failures during file observation, outside-root resolution, detected changes and timeout. Resource-specific failures may instead be fields inside a successful operation. A deadline failure discards the operation result.

## Verification and remaining work

Disposable local fixtures cover native text/directory/package types, tagged names, a real bookmark alias whose target is outside root, leaf symlinks, scoped following, source-byte preservation, unchanged tag attributes, revision rejection and detected resource-time changes. Synthetic fixtures cover dataless/special-file hook avoidance, partial resource errors and unsupported platform defaults. These checks do not establish true File Provider/TCC/unmount behavior or execution on an older macOS release.

Package recognition does not change traversal: list remains one level; search's default depth remains 1; an explicitly deeper search treats packages as ordinary directories. Search kind filters remain filesystem kinds, without UTType/tag filters. Spotlight content search and richer provider state remain in #40. [Finder context/reveal](files-finder.md) have separate commands; actual reveal UI delivery still needs desktop acceptance. Previews belong to #41 and file-management writes to #42.
