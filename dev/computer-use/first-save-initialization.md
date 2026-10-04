# First-save initialization and native-app contrasts

The disabled inactive-Space Save panel is reproducible, but these results do not establish a Cueward input/AX defect or an AppKit root cause. A complete app bundle and ordinary launch-delegate initialization did not remove it. Unmodified TextEdit produced a new artifact through existing generic interfaces; no Save panel was observed. [#33](https://github.com/Termdock-dev/cueward/issues/33) remains incomplete.

The [sanitized aggregate](results/2026-10-04-first-save-initialization.json) records macOS 27.0.1, build 26A434, arm64, three online displays. Raw AX trees, targets, desktop identities, filenames, bookmarks, traces and artifact backups remain private. No product code, disabled-control guard or input route was changed.

## Synthetic initialization controls

These operator diagnostics use the existing [NSDocument fixture](document-fixture.swift) and [run-loop observer](document-observer.swift). They do not count as fresh-agent acceptance. Only disposable output locations/default filenames are prepared; no enabled state, event acceptance, activation or Space-switch behavior is overridden.

The before-run contrast uses the same compiled accessory executable bare and inside an `.app`. The bundle declares an executable, unique bundle ID, `APPL`, `NSApplication`, `LSUIElement` and a plain-text document type mapped to the fixture's document class. Configuration is supplied through environment variables rather than positional paths: AppKit interpreted the latter as document-open requests in preliminary bundle runs, producing an unexpected alert. Those preliminary bundle runs are excluded.

Additional read-only observations cover bundle/activation policy, document class/type/edit state, key/main window and prepared panel name/directory/URL. The lifecycle contrast creates the document window from `applicationDidFinishLaunching` and suppresses an additional automatic untitled window. Save behavior and sender interfaces remain unchanged. Each inactive case moves only its owned document to an existing inactive Space, edits it and requests document Save once. Fresh shallow app-root reads avoid exhausting the 500-node limit in an expanded file browser; unrelated descendants remain truncated and are not claimed to have been fully inspected. Panel Save is never dispatched when its observed control is disabled or unavailable.

| Initialization | Context | Save enabled reads | Exact artifact | Maximum sample gap | Stationary check |
| --- | --- | --- | --- | ---: | --- |
| Bare executable, before run | Inactive | 6 disabled | No; 0 panel Save dispatches | 79.4 ms | Failed |
| Complete bundle, same executable | Inactive | 6 disabled | No; 0 panel Save dispatches | 103.8 ms | Failed |
| Complete bundle, same executable | Visible | 1 enabled | Yes | 61.7 ms | Failed |
| Bundle, launch delegate | Inactive | 3 disabled | No; 0 panel Save dispatches | 77.2 ms | Failed |
| Bundle, launch delegate | Visible | 1 enabled | Yes | 63.5 ms | Passed at sampled resolution |

The inactive cases allow at least ten seconds of fresh read-only panel observation, rather than stopping at the first disabled read. Foreground and all visible-Space mappings stayed sampled-stable, with zero receiver activations in these five cases. All except the delegate-visible case recorded unattributed pointer movement; bundle-inactive also exceeded the 100 ms coverage bound. One run per case does not establish a platform-wide rule. A further independent raw-AX cross-check stopped after its document AXPress returned an uncertain error, before observing a panel; it supplies no cross-check conclusion and was not replayed. A separate regular-policy direct-executable control activated during the trial and was stopped; it is invalid as a background compatibility control, not a remedy.

## Unmodified TextEdit contrast

TextEdit 1.21 was not patched, swizzled or given an app-specific adapter. Setup first verified no running instance, no saved application state and an empty autosave directory. This preflight is a restriction of this diagnostic, not a safe-isolation guarantee for launching arbitrary apps with `--new-instance`. Existing user documents and instances were not used. Its initially empty document was created before being moved to the inactive Space, so creation while already inactive remains unverified.

Generic `app set-value` confirmed the supplied multiline Unicode editor value. The observed File Save menu item was disabled and was not AX-pressed. A separately planned single `window key --key s --modifiers command`, bound to a fresh snapshot and the verified keyboard window, posted one down/up pair. There was no disabled-panel press, automatic keyboard fallback or uncertain Save replay.

Subsequent independent AX document-URL observation found an app-selected RTF artifact. Its 500 bytes were copied and compared independently; `textutil` decoded exactly the expected text. A separate disposable [reader app](../../crates/adapter-macos/src/apps/open_live_fixture.swift), through generic `app open`, read the original artifact's exact RTF bytes, with zero reader activations. This reader is synthetic, not a second unintegrated-app acceptance run.

No Save panel was observed, and no filename or destination was selected by the operator. TextEdit used its default iCloud document directory. Command-S versus automatic-save causality was not distinguished. This is evidence that an existing generic route can accompany a native new-document artifact, not a fix for the disabled-panel case or controlled-destination acceptance. Do not deploy it as an automatic retry/fallback.

The writer's 266,017.1 ms marked interval sampled one foreground PID, unchanged visible Spaces, no activation events and confirmed inactive document membership after artifact creation. It recorded 327 pointer positions and a 238.3 ms maximum gap, so stationary isolation failed. The reader's separate 668.2 ms interval also failed stationary isolation due to pointer movement. These are separate intervals, not continuous end-to-end coverage. The first TextEdit setup had rejected a stale Space-move identity after window bounds changed and performed no edit or Save; it is not replaced by a retrospective pass.

Only the newly created app instances were stopped. The generated document and its sole newly created autosave-index entry were removed only after byte-exact, readable private backups and ownership checks; the previously empty autosave directory was restored to empty. No pre-existing document was overwritten or closed.

## Consequence for implementation

Do not remove the disabled-control guard or substitute activation, visible-Space warm-up or forced panel input. The fixture failures, initialization contrasts and native artifact result narrow the investigation but do not prove a sender defect. The remaining work is the original inactive-first Save-panel case, creation/destination selection while inactive, and one continuously observed, controlled no-interference cross-app task. Prior [context results](first-save-context.md) and [fresh-agent failures](agent-session.md) retain their original status.
