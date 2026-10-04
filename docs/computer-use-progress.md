# Computer-use progress

Updated 2026-10-04, Asia/Taipei. Repository history, open issues and the linked aggregate diagnostics are the sources for this handoff. Raw desktop evidence remains local.

## Merged foundation

PRs #15–#30 provide cross-Space window discovery and snapshots, AX subtree/menu/application exploration, guarded background keyboard and pointer input, native window identity for keyboard dialogs, condition waits, background application/file discovery, Space observation targets and native desktop creation. PR #30 was merged on 2026-09-27. The source and current contracts are documented in [window observation](window-observation.md), [background input](background-input.md), [app exploration](app-exploration.md) and [Space management](space-management.md).

PR #68 merged on 2026-10-04. It adds a [bounded multi-display observer](../dev/computer-use/desktop-observer.md), synthetic diagnostics, actual fresh-agent transport/runs and corrected external-content/malformed-request handling. It does not repair raw-pointer acceptance or close the remaining computer-use issues.

## Remaining computer-use issues

| Issue | Current evidence and next requirement |
| --- | --- |
| [#31](https://github.com/Termdock-dev/cueward/issues/31) Physical/background isolation | The [actual human round](../dev/computer-use/physical-overlap.md) retained typing and background text/click effects, but ordinary canvas acceptance failed and pointer attribution/manual-finish coverage was incomplete. Corrected idle scroll effects do not replace physical overlap. Accepted gestures, another controlled human round and mid-flight foreground activation remain unverified. |
| [#32](https://github.com/Termdock-dev/cueward/issues/32) WebView/canvas | The [ordinary raw-pointer matrix](../dev/computer-use/webview-context.md) reproduced no click/drag effects on visible and inactive Spaces; receiver-patched controls are not a fix. A later [AX contrast](../dev/computer-use/webview-ax.md) exposed page buttons after a cold read and demonstrated exactly-once effects through existing AXPress. Pointer movement left full isolation unverified; generic accepted canvas drag/interruption and real-app coverage remain open. |
| [#33](https://github.com/Termdock-dev/cueward/issues/33) First save on an inactive Space | The [context matrix](../dev/computer-use/first-save-context.md) reproduced disabled inactive-first Save controls with no artifact. Visible/warm controls produced exact files but failed stationary pointer checks and exceeded the 100 ms sampling bound. Warm creation is not the original task's solution. |
| [#34](https://github.com/Termdock-dev/cueward/issues/34) Resources retained across lock | The [owned retained-resource probe](../dev/computer-use/lock-resources.md) now records advancing unlocked AX/stream/screenshot observations and independently timed AXPress effects. Actual lock/unlock phases, CG input targets and stream renewal still require research; product locked-session guards remain enabled. |
| [#35](https://github.com/Termdock-dev/cueward/issues/35) Snapshot differences and performance | PR #39 merged and the issue closed. `window diff --previous`, selective wait attributes and the [benchmark and samples](../dev/computer-use/README.md) are integrated. Precompiled input lost one of five pairs, so no production speedup was adopted. |
| [#36](https://github.com/Termdock-dev/cueward/issues/36) Cross-app task acceptance set | The [offline framework](../dev/computer-use/task-acceptance.md) retains all eight tasks. Three historical fresh-agent attempts remain zero passed, three failed and five unverified, with the [transport record](../dev/computer-use/agent-session.md) preserving their corrected-later trust-boundary limitation. The new [native task runner](../dev/computer-use/native-agent-tasks.md) exposed an optional `AXIdentifier` capability bug. After its fix, two fresh Agents produced exact document/calculation artifacts, but interference failures leave that separate run at zero passed, two failed and six unverified. Cross-app/dialog/canvas and real-app coverage remain incomplete. |

Issues #8, #10 and #11 retain the older Safari capability/architecture comparisons. Their remaining extension/private-API boundaries are separate from the native computer-use acceptance work; this handoff does not turn every comparison gap into an implementation commitment.

## 2026-10-01 continuation

`feat/window-snapshot-diff` also fixes a wait failure discovered by the benchmark: role-only waits queried irrelevant AXDescription attributes and could fail on a native text receiver. The deterministic regression failed before the fix and passed after it. Required selector attribute failures still fail the query; all five opt-in wait desktop tests passed. The new background snapshot-diff desktop test passed. Release build and the full test suite passed; strict Clippy remains blocked by pre-existing warnings, and normal Clippy identified no remaining warning in the added code.

The benchmark's precompiled input experiment remains a failed compatibility observation, not a production regression or a delivered optimization. The report preserves the missing effect, paired releases, unchanged fixture text and the local timing distribution. Its cause is still unconfirmed. Full isolation and locked-session claims require the separate #31/#34 experiments.

Later desktop preparation found the session locked while Screen Recording access remained granted. WebView capture attempts in that state provide no compatibility verdict. Their receivers were terminated without sending pointer input. The prepared probes now reject a locked session before creating a receiver; unlock is required to resume #32/#33. Physical input and controlled lock/unlock participation remain separate prerequisites for #31/#34.

## After PR #68

The ordinary WebView AX contrast continues #32 without a product routing change. Four fresh receiver cases, two read-only and two action cases, preserve the seven-to-twelve-node transition, exact single-click effects and every failed stationary desktop check. This establishes a scoped existing semantic-button route and a bounded reobservation workflow, not a generic canvas repair or a new fresh-agent pass. The next functional gate remains an unmodified accepted drag/release route with the existing target and interruption protections intact.
