# Cross-app task acceptance set

Prepared for issue [#36](https://github.com/Termdock-dev/cueward/issues/36), 2026-10-01. This document defines repeatable tasks and independent completion checks. It is not a report that the agent has completed the tasks.

The [offline preparation and evaluation framework](task-acceptance.md) generates fresh synthetic workspaces and reports every task, including blocked and unrun cases. It needs independently supplied receiver and execution evidence; it does not drive the agent or collect desktop observations. No actual fresh-agent runs are recorded yet.

## Shared setup and evidence

Use a unique temporary directory, synthetic text and newly created receiver processes. Record their owned PIDs/windows before operations and restrict cleanup to those resources. Use an unlocked session with Accessibility and Screen Recording permissions. An unavailable prerequisite is `blocked`, not a tool compatibility failure or a passed task. Existing user apps, documents, tabs and Spaces are not cleanup targets.

Give the agent only the task goal, synthetic content, permitted app/process scope and disposable destination. Do not supply AX refs, selectors, coordinates, recorded click sequences or app-specific adapters. The agent must discover current app/window roots, choose current controls and reobserve after changes. Fixture/probe scripts are independent regression and diagnostic tools; their scripted actions do not count as a fresh agent acceptance run.

Record command count, task wall time, tool execution time and observer sample intervals separately. Preserve action receipts and independent result evidence without counting `sent_unverified` as completion. Collect foreground, target activation, pointer and per-display visible-Space observations throughout the execution interval, distinguishing setup and cleanup. Endpoint comparisons alone leave transient interference unverified. Raw personal desktop data remains local; publish only synthetic content and aggregate results.

## Tasks

| Task | Resettable setup and goal | Independent completion check |
| --- | --- | --- |
| New native document | Start an empty disposable NSDocument receiver; ask the agent to enter the supplied multiline Unicode text and save the new document into the given disposable directory on an inactive Space. | File exists, UTF-8 bytes equal the requested text, save submitted once, and required interference observations are complete. A disabled initial save panel remains failed/unavailable. |
| Existing native document | Copy a synthetic UTF-8 file into a unique task directory; open it in a new permitted native document app instance and request a specific edit and save. | The copied file's bytes equal the expected edit. Preserve the source fixture and verify no other file was modified. |
| Cross-app artifact | Use the actual saved artifact from either document task; ask the agent to open it in a different freshly created reader process and confirm its content. | Recipient independently reports the same file and exact contents; producing the file and requesting open do not substitute for recipient evidence. |
| Window/dialog change | Start two owned native windows; request an edit in a named window, open its file dialog, then continue through the changed window hierarchy. | Native window identities bind target and bystander documents. Ordered observations identify current document roots, the save dialog and its owner/parent, then the resumed target with no dialog present. Only the intended document changes; fresh snapshot names alone are insufficient. Stale-parent or wrong-keyboard-window rejection is recorded, not bypassed. |
| Calculation | Use a new permitted calculator/document app to calculate `19.95 × 3 + 7.50 × 2 − 5.00` and save the two-decimal result. | Independently calculate with Decimal and compare the saved UTF-8 bytes to `69.85` plus a newline. |
| Ordinary WebView first click | Start a newly loaded ordinary WKWebView counter, with no first-mouse or input behavior override; ask for one counter increment while it stays in the background. | Counter increases exactly once. A double click or automatic replay cannot turn a failed first click into success. |
| Canvas drag | Start the ordinary canvas with a fresh object position, then request a specified displacement. Repeat separately with a controlled interruption. | Object displacement and selected state match; down/up and interrupted release are observed, with no automatic replay. DOM event delivery without displacement is not a completed drag. |

Reset by terminating only the owned receiver instances and preparing a new unique directory. Preserve needed evidence and reports before removing an old owned task directory or its generated snapshots. The framework does not launch or terminate processes. Recompile/relaunch fixtures for the next run. Never reuse a prior input or AX target across resets. A task that needs a real installed app must use a new explicitly identified instance and preserve its existing user-owned documents and settings.

## Support matrix at preparation

| Interface / path | Current evidence | Acceptance status |
| --- | --- | --- |
| Native AppKit AX and targeted keyboard/pointer | Existing primitive regression tests and document/application foundations are merged. Matching waits and background snapshot differences passed this continuation's desktop checks. | Individual primitives verified; the complete task set remains unrun. |
| Existing native file / background open | Prior background open/edit/save and cross-app read cases; merged `app open` receiver regression. | Broader repeatable task report still required. |
| New-document save panel | Known disabled-controls limitation; a standard NSDocument first-save and cross-app probe has been prepared. | Unverified pending unlocked desktop; real-app and continuous Space cases remain required. |
| Ordinary WKWebView / canvas | An unmodified-behavior receiver and independent effect probe have been prepared. Initial capture attempts occurred while the desktop was locked and sent no input. | Unverified; do not classify those attempts as WebView incompatibility. |
| Concurrent physical input | Prior physical typing case and synthetic interruption/release regressions. | Full overlapping keyboard/mouse task requires user participation under #31. |
| Retained resources across lock | No new retained-resource experiment in this continuation. | Unverified; controlled lock/unlock and freshness checks remain under #34. |
| Precompiled input experiment | One of five reported key pairs produced no receiver effect; interpreted and CLI samples each delivered five of five. | Experimental input route failed verification and was not adopted. |

Classify unsuccessful runs as prerequisite unavailable, tool observation/dispatch failure, application rejection, ambiguous/partial observation, stale target, missing artifact, interference observed, or agent decision error. Preserve unresolved and unverified cases explicitly rather than omitting them from the denominator. Full completion of #36 requires actual agent runs and the complete interference trace, not this prepared task definition.
