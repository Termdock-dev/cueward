# Scoped Finder context and reveal

`cueward files finder context` reads the front Finder window's target and Finder's current selection. `cueward files finder reveal` explicitly requests that Finder display and select a scoped item. These extend [file operations](files.md) for [issue #40](https://github.com/Termdock-dev/cueward/issues/40). Check `cueward files finder --help` on the installed binary before use; this source revision can be newer than main or a published release.

```bash
cueward files finder context --root /Users/me/Documents
cueward files finder context --root /Users/me/Documents --max-items 500 --timeout-ms 5000
cueward files finder reveal --root /Users/me/Documents --path Reports/report.txt --expected-version '<previous file version>'
```

Both commands require an explicit absolute directory root. Reveal also requires an explicit relative path without `..`. A selected root may itself be a directory symlink. Finder-native paths are mapped into canonical root, or the explicitly supplied root spelling, before file observation. The worker never broadens root or opens selected file contents.

## Read context

The result is `{"Ok":{"operation":"finder_context","result":{...}}}` inside the existing `<external source="cueward/files">` JSON wrapper. It contains:

| Field | Meaning |
| --- | --- |
| `root` | Existing FileInfo observation for the selected root. Its revision does not bind Finder UI state. |
| `finder_pid` | Observed running Finder PID, or null if no instance was found. |
| `window_count` | Finder's reported ordinary window count, or null if Finder was not running. |
| `front_window` | `{id, location}` for the first Finder window, or null when none was reported. This is Finder's window order, even when another app owns the foreground. |
| `selection` | Array in Finder's selection order, including per-item errors/outside-scope items. Null when Finder was not running; `[]` only when the query returned an empty selection. |
| `selection_complete` | Whether the returned selection array covers the query's entire selection. This does not mean every item has available file information or that the observations are atomic. |
| `activation_requested` | False. Context does not send activate, reveal or selection-setting commands. |
| `foreground` | Optional before/after foreground PIDs and `foreground_changed`. Missing observations produce null, not an invented unchanged result. |

Each location/selection item has a `status`:

| Status | Meaning |
| --- | --- |
| `available` | Contains `relative_path` and existing `file` FileInfo. Use that path with the same root for info, metadata or read. |
| `outside_scope` | The path is outside the selected root. No path/name/file metadata is returned for that item. |
| `error` | Contains existing `{code, message}`. Preserve missing-file, permission, unsupported URL and encoding failures as gaps. |

The native query observes Finder's current target/selection URLs to determine scope; filesystem information is read only for scoped paths. Virtual locations without a usable local file URL, remote-host file URLs, invalid percent escapes, non-UTF-8 paths and individual URLs over 16 KiB produce item errors. Percent decoding is strict and performed once. Leaf symlinks are described without following them; symlink traversal in a selected path is rejected. Native diagnostics remain external data.

`--max-items` defaults to 100 and accepts 1..500. If selection exceeds it, the command returns `scan_limit` with no successful partial array. This bounds accepted selection results; Finder/JXA may allocate the native selection list before checking its size. Item metadata failures preserve the rest of the selection. Apple Events query failures, detected Finder PID changes and root changes discard the operation instead of returning a successful empty selection.

Finder's installed scripting dictionary defines `selection` as the selection in the frontmost Finder window and provides each item's URL and a Finder window's target. The JXA query checks running state and does not request activation. It needs permission to send Apple Events to Finder; `-1743` maps to `permission_denied`. Check the invoking host's access under System Settings > Privacy & Security > Automation. The implementation does not use Accessibility or require Swift.

## Reveal is an explicit desktop action

Reveal uses AppKit's [activateFileViewerSelectingURLs](https://developer.apple.com/documentation/appkit/nsworkspace/activatefileviewerselecting%28_%3A%29) API. It can launch/activate Finder, open a window and change selection. Root constrains the submitted item and worker observations; it does not confine Finder's displayed parent directory or its other windows. Run it only when the user requested that desktop action.

The command accepts a regular file or directory. An unfollowed symlink/special file is rejected; explicit `--follow-links` may resolve the requested path only inside canonical root. Dataless items are rejected before submission. `--expected-version` binds to a prior FileInfo revision. Worker file/root checks run before submission, including after acquiring locks.

Submission holds the global `finder-reveal.lock` and, when Finder is already running, the existing `input-<pid>.lock` shared with other Cueward input actions. A busy lock or detected recipient change prevents submission. Locks cover the worker's submission/post-check period; they do not extend through an asynchronous Finder UI completion.

A success is `{"Ok":{"operation":"finder_reveal","result":{...}}}` with:

| Field | Meaning |
| --- | --- |
| `status` | Always `sent_unverified`. The native API has no completion acknowledgement proving the intended window/selection appeared. |
| `file` | Scoped observation used for submission. |
| `activation_requested`, `selection_change_requested` | True. |
| `foreground` | Immediate before/after observations; an unchanged PID does not prove Finder will remain in the background. |
| `post_check` | `Ok(FileInfo)` if file/root checks passed after submission, otherwise `Err({code,message})`. This checks the filesystem, not UI delivery. |

If a post-check detects a change, the outer result remains `sent_unverified` because submission already occurred. A timeout or invalid worker response can also lose the result after delivery; inspect Finder before retrying. There is no automatic retry or foreground restoration. A fresh context/independent observation can verify the effect when permissions and scope permit it.

## Deadlines, provider boundaries and verification

The dedicated worker uses the existing 16 KiB request/16 MiB response contract and `--timeout-ms` default 10,000, range 1..30,000. JXA inherits its worker process group so the parent deadline stops both owned processes. The worker applies the existing thread-local no-materialization policy to file observations and sends no provider download API request. Finder itself is a separate process; its preview/navigation behavior is outside that guard. `not_dataless` does not establish network/provider availability.

All names, paths, URLs and diagnostics are external content. JSON escapes preserve decoded strings, including embedded external markers. File/root revisions and PID checks detect observed changes, without an atomic UI/filesystem snapshot or a sandbox against malicious concurrent path swaps. Window target and selection queries are sequential and can reflect different instants.

Tests use disposable files and the production scope/version/lock/result paths with mocked native desktop calls. JXA tests execute the production query in the system interpreter with only its Finder boundary replaced; they cover URL preservation, query/item errors, no-window/empty selection and budget rejection. CLI tests stop invalid requests before desktop dispatch. A real read-only context probe returned scoped exclusions with unchanged foreground PID. Actual reveal UI delivery, true TCC denial, File Provider behavior and volume unmount scenarios remain unverified. #40 stays open for those acceptance scenarios, Spotlight content search and richer provider state; previews/writes remain #41/#42.
