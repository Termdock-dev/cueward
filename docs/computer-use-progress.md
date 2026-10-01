# Computer-use progress

Updated 2026-10-01, Asia/Taipei. Repository history and open issues are the source for this handoff; the project memory search returned no relevant prior entries.

## Merged foundation

PRs #15–#30 provide cross-Space window discovery and snapshots, AX subtree/menu/application exploration, guarded background keyboard and pointer input, native window identity for keyboard dialogs, condition waits, background application/file discovery, Space observation targets and native desktop creation. PR #30 was merged on 2026-09-27. The source and current contracts are documented in [window observation](window-observation.md), [background input](background-input.md), [app exploration](app-exploration.md) and [Space management](space-management.md).

## Remaining computer-use issues

| Issue | Current evidence and next requirement |
| --- | --- |
| [#31](https://github.com/Termdock-dev/cueward/issues/31) Physical/background isolation | Existing synthetic receivers and interrupted-delivery tests cover individual primitives. A prior physical typing run is documented. The complete overlapping physical-keyboard/mouse run with continuous foreground/pointer/visible-Space observations still requires user participation. |
| [#32](https://github.com/Termdock-dev/cueward/issues/32) WebView/canvas | Native pointer receivers have regression coverage. The ordinary WKWebView click/canvas probe and reporting regressions are merged in PR #38; its desktop attempts stopped before input while the session was locked. Actual first-click and drag outcomes remain unverified. A double click cannot substitute for first-click success. |
| [#33](https://github.com/Termdock-dev/cueward/issues/33) First save on an inactive Space | Background open/edit/save of existing files and cross-app reading have prior success. The standard NSDocument first-save/cross-app probe and modal observer regression are merged in PR #37; desktop execution remains unverified pending unlock. Initial save-panel disabled controls remain unresolved. |
| [#34](https://github.com/Termdock-dev/cueward/issues/34) Resources retained across lock | Requires controlled lock/unlock participation and freshness evidence for retained/new AX, capture and input resources. Product locked-session guards remain enabled. |
| [#35](https://github.com/Termdock-dev/cueward/issues/35) Snapshot differences and performance | PR #39 merged and the issue closed. `window diff --previous`, selective wait attributes and the [benchmark and samples](../dev/computer-use/README.md) are integrated. Precompiled input lost one of five pairs, so no production speedup was adopted. |
| [#36](https://github.com/Termdock-dev/cueward/issues/36) Cross-app task acceptance set | The [offline framework](../dev/computer-use/task-acceptance.md) creates unique/resettable synthetic workspaces and checks artifacts, calculation, receiver effects and supplied sampled interference traces. Reports retain all eight tasks in their denominator. Actual fresh-agent runs and a full multi-display desktop observer remain incomplete; diagnostic/simulated results cannot count as passed agent tasks. |

Issues #8, #10 and #11 retain the older Safari capability/architecture comparisons. Their remaining extension/private-API boundaries are separate from the native computer-use acceptance work; this handoff does not turn every comparison gap into an implementation commitment.

## This continuation

`feat/window-snapshot-diff` also fixes a wait failure discovered by the benchmark: role-only waits queried irrelevant AXDescription attributes and could fail on a native text receiver. The deterministic regression failed before the fix and passed after it. Required selector attribute failures still fail the query; all five opt-in wait desktop tests passed. The new background snapshot-diff desktop test passed. Release build and the full test suite passed; strict Clippy remains blocked by pre-existing warnings, and normal Clippy identified no remaining warning in the added code.

The benchmark's precompiled input experiment remains a failed compatibility observation, not a production regression or a delivered optimization. The report preserves the missing effect, paired releases, unchanged fixture text and the local timing distribution. Its cause is still unconfirmed. Full isolation and locked-session claims require the separate #31/#34 experiments.

Later desktop preparation found the session locked while Screen Recording access remained granted. WebView capture attempts in that state provide no compatibility verdict. Their receivers were terminated without sending pointer input. The prepared probes now reject a locked session before creating a receiver; unlock is required to resume #32/#33. Physical input and controlled lock/unlock participation remain separate prerequisites for #31/#34.
