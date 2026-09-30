# Computer-use progress

Updated 2026-10-01, Asia/Taipei. Repository history and open issues are the source for this handoff; the project memory search returned no relevant prior entries.

## Merged foundation

PRs #15–#30 provide cross-Space window discovery and snapshots, AX subtree/menu/application exploration, guarded background keyboard and pointer input, native window identity for keyboard dialogs, condition waits, background application/file discovery, Space observation targets and native desktop creation. PR #30 was merged on 2026-09-27. The source and current contracts are documented in [window observation](window-observation.md), [background input](background-input.md), [app exploration](app-exploration.md) and [Space management](space-management.md).

## Remaining computer-use issues

| Issue | Current evidence and next requirement |
| --- | --- |
| [#31](https://github.com/Termdock-dev/cueward/issues/31) Physical/background isolation | Existing synthetic receivers and interrupted-delivery tests cover individual primitives. A prior physical typing run is documented. The complete overlapping physical-keyboard/mouse run with continuous foreground/pointer/visible-Space observations still requires user participation. |
| [#32](https://github.com/Termdock-dev/cueward/issues/32) WebView/canvas | Native pointer receivers have regression coverage; ordinary WKWebView first-click and actual canvas drag outcomes still require a reproducible compatibility run. A double click cannot substitute for first-click success. |
| [#33](https://github.com/Termdock-dev/cueward/issues/33) First save on an inactive Space | Background open/edit/save of existing files and cross-app reading have prior success. Initial save-panel disabled controls remain unresolved; keyboard-window binding alone does not establish acceptance. Next: disposable new document and independently read saved artifact. |
| [#34](https://github.com/Termdock-dev/cueward/issues/34) Resources retained across lock | Requires controlled lock/unlock participation and freshness evidence for retained/new AX, capture and input resources. Product locked-session guards remain enabled. |
| [#35](https://github.com/Termdock-dev/cueward/issues/35) Snapshot differences and performance | Implemented `window diff --previous`, documented its states and coordinate rules, and added real-PNG/unit and background desktop coverage. The [benchmark and samples](../dev/computer-use/README.md) separate helper work, serialization, startup/transport and CLI timing. Interpreted and CLI key samples passed; precompiled input lost one of five pairs, so no production speedup was adopted. Awaiting PR review/integration. |
| [#36](https://github.com/Termdock-dev/cueward/issues/36) Cross-app task acceptance set | The capability foundations exist, but a complete resettable task set, independent artifact checks and platform/interface support matrix remain to be assembled. Native document/save and ordinary WebView results must be reported as completed, unsupported or unverified individually. |

Issues #8, #10 and #11 retain the older Safari capability/architecture comparisons. Their remaining extension/private-API boundaries are separate from the native computer-use acceptance work; this handoff does not turn every comparison gap into an implementation commitment.

## This continuation

`feat/window-snapshot-diff` also fixes a wait failure discovered by the benchmark: role-only waits queried irrelevant AXDescription attributes and could fail on a native text receiver. The deterministic regression failed before the fix and passed after it. Required selector attribute failures still fail the query; all five opt-in wait desktop tests passed. The new background snapshot-diff desktop test passed. Release build and the full test suite passed; strict Clippy remains blocked by pre-existing warnings, and normal Clippy identified no remaining warning in the added code.

The benchmark's precompiled input experiment remains a failed compatibility observation, not a production regression or a delivered optimization. The report preserves the missing effect, paired releases, unchanged fixture text and the local timing distribution. Its cause is still unconfirmed. Full isolation and locked-session claims require the separate #31/#34 experiments.
