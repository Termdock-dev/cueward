# iCloud state and explicit download requests

`cueward files cloud status` reads per-field iCloud resource state without opening file contents or requesting a download. `files cloud download` explicitly asks macOS to download one selected iCloud file. These extend [file operations](files.md) for [issue #40](https://github.com/Termdock-dev/cueward/issues/40). Check `cueward files cloud --help` on the installed binary; source documentation can be newer than a published release.

```bash
cueward files cloud status --root /Users/me/Documents --path Reports/report.pdf
cueward files cloud download --root /Users/me/Documents --path Reports/report.pdf --expected-version '<fresh file version>' --max-bytes 33554432
```

## Scope and field availability

Root is an explicit absolute directory. Path is relative to canonical root and cannot contain `..`; status defaults to `.` while download requires an explicit path. The existing symlink policy applies: an unfollowed leaf link retains its FileInfo with `not_applicable` cloud fields. Explicit `--follow-links` resolves only within root. Special files also have `not_applicable` fields. A dataless directory cannot be a root or traversed ancestor, but a leaf placeholder can be observed under the worker's no-materialization policy.

A status success is `{"Ok":{"operation":"cloud_status","result":{...}}}` inside `<external source="cueward/files">`. The result contains `file` (FileInfo), `resources`, `provider_coverage: icloud_keys_other_providers_unknown`, and `download_requested_by_operation: false`.

Each resource uses the existing `available/value`, `unavailable`, `not_applicable`, `unsupported`, or `error/error` contract. Missing values remain unavailable; a failed query preserves NSError domain, code and message. An outer Ok does not mean every field was available. Status string values and NSError-valued resource domain/message pairs are bounded to 65,536 UTF-8 bytes, with bound/type failures reported as field errors rather than truncated success. Native query-failure diagnostics retain the shared metadata contract; the worker response also has an overall bound.

| Resource | Value when available |
| --- | --- |
| `is_ubiquitous` | Whether Foundation reports membership in iCloud. False does not identify a local-only file or exclude another provider. |
| `downloading_status` | Object with `state: not_downloaded`, `downloaded`, `current`, or `other` with the unfamiliar native `value`. Downloaded is distinct from current. |
| `is_downloading` | Whether Foundation currently reports an active download. |
| `download_requested` | Whether a download was requested; this flag alone does not establish an ongoing transfer. |
| `is_uploaded`, `is_uploading` | Reported upload state. Neither establishes current-byte readability. |
| `has_unresolved_conflicts` | Reported iCloud conflict state. |
| `downloading_error`, `uploading_error` | Available value contains the reported NSError's domain/code/message. Absence remains unavailable, not proof that no error occurred. |

If `is_ubiquitous` is available/false, dependent fields are `not_applicable`. If membership is unknown or fails, dependent fields remain `unavailable`. Resource keys are queried only when membership is available/true. A local disposable file on macOS 27.0.1 returned no membership value; this is preserved as unavailable rather than invented false.

The native keys are Apple's [isUbiquitousItemKey](https://developer.apple.com/documentation/foundation/urlresourcekey/isubiquitousitemkey), [ubiquitousItemDownloadingStatusKey](https://developer.apple.com/documentation/foundation/urlresourcekey/ubiquitousitemdownloadingstatuskey) and related iCloud resource keys. They do not provide uniform third-party File Provider state. Apple's [user-visible file identifier API](https://developer.apple.com/documentation/fileprovider/nsfileprovidermanager/getidentifierforuservisiblefile(at:completionhandler:)) is scoped to a provider's own extension and is not a general provider discovery API for this CLI. Generic provider state and downloads remain unsupported/unknown.

Status accepts optional `--expected-version` from a prior FileInfo. File/path/root observations are checked again before success; detected changes discard the result. The metadata revision is not a content hash or atomic cloud-state snapshot. Native resource fields are sequential observations, and provider state may change without changing the file revision. The existing concurrent-path-swap limitations apply.

## Explicit download and receipt

Download accepts one regular file with available/true iCloud membership. Directories, unfollowed links, special files and unknown/non-iCloud membership are rejected before calling the download API. Obtain a fresh cloud status first, then supply its `file.version` with required `--expected-version`; this token guards metadata changes and is not authorization to download an unrelated file.

`--max-bytes` defaults to 33,554,432 and accepts 1..268,435,456. It bounds the reported file size at preflight, not network traffic or the final downloaded size. It cannot cancel a provider transfer, enforce a byte cap after submission, or reserve storage. Oversized reported files return `scan_limit` without submission.

The implementation calls [startDownloadingUbiquitousItem(at:)](https://developer.apple.com/documentation/foundation/filemanager/startdownloadingubiquitousitem(at:)) once. This is an asynchronous request, not a wait for download completion. It does not open Finder, alter its selection or request app activation.

A success is `{"Ok":{"operation":"cloud_download","result":{...}}}`:

| Field | Meaning |
| --- | --- |
| `operation_id` | Unique receipt ID; not a durable provider job or cancellation token. |
| `status` | `sent_unverified` when the API accepted a new request; `already_current` when native current state and not_dataless were observed; `already_requested` when an active download was observed. The latter two make no new API call. |
| `before` | CloudStatus used for preflight. Its download flag remains false because observation itself requested nothing. |
| `max_bytes` | Applied reported-size preflight limit. |
| `download_requested_by_operation` | True only when this operation successfully called the native download API. |
| `completion_verified` | Always false. No receipt status establishes completed/current readable bytes. |
| `post_check` | Ok(CloudStatus) or Err(code/message). Checks scoped identity/path/root after submission; it is not transfer completion verification. |

The operation holds a shared download lock through submission and post-check, not through the asynchronous transfer. It reobserves the source revision immediately before submission. A changed root/item after submission is retained as a failed post_check inside the receipt; an outer Ok can therefore contain a submitted request and an observation failure. Normal download metadata changes are permitted after submission while item identity/path must remain the same.

Both commands use a supervised worker, a 16 KiB request cap, a 16 MiB response cap and `--timeout-ms` 1..30,000 (default 10,000). For download, the deadline bounds submission/observation, not provider activity. Killing the owned worker process group cannot cancel an already accepted request. Timeout, I/O or invalid worker-response errors warn that a download may already have been requested. Native submission errors retain domain/code/message. There is no automatic retry, cancellation, directory download or durable job tracker.

On uncertainty, observe fresh cloud status before deciding whether to retry. Do not silently download as a fallback for `files read`, metadata, search or preview. After reported current state, use an independently guarded read to establish the requested content is readable; keep any resulting unavailable/changed/error separate from successful submission.

## Verification and remaining acceptance

Tests exercise guarded submission, no implicit download, unknown membership, active/current handling, dataless/current disagreement, size/version/scope/type rejection, locking, native errors without retry, before/after changes and uncertain deadline delivery. CLI integration uses disposable local files and symlinks to verify external-data preservation and rejection before the real download API. Native decoder tests use Foundation status constants and NSError values. On macOS 27.0.1, a release probe confirmed unavailable local membership, version/scope/membership rejection, an unfollowed broken leaf link, preserved Unicode/newline/external paths, unchanged source SHA-256, and unchanged independent foreground/Finder window/selection observations.

These checks do not establish a completed iCloud transfer, real placeholder metadata behavior, offline/provider conflicts, TCC denial, volume unmounts or older macOS support. No personal cloud file was downloaded for these checks. #40 remains open for desktop/provider acceptance and generic provider limitations; [PDF/image/Quick Look preview](https://github.com/Termdock-dev/cueward/issues/41) and [file management](https://github.com/Termdock-dev/cueward/issues/42) remain subsequent batches.
