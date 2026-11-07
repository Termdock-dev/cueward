# Physical/background input overlap diagnostic

Issue #31 remains open. One actual operator-assisted round was recorded on 2026-10-03, followed by idle diagnostics on 2026-10-04 (Asia/Taipei). This is a scripted diagnostic, not fresh-agent acceptance. No production input route or guard is changed.

## Run and ownership

Use an unlocked macOS desktop with existing Accessibility, Screen Recording and input-posting permissions. Use a private local evidence directory:

```sh
python3 -B dev/computer-use/physical-overlap-probe.py \
  --cli /absolute/path/to/cueward --parent /private/local/evidence
```

The runner creates two new background AppKit receivers and moves only their windows to one existing inactive user Space. It independently verifies current native memberships. It creates no Space, switches no Space, activates no app and posts no global input. Ordinary user App changes during preparation are recorded rather than imported into the fixed-foreground execution interval; receiver/visible-Space/window changes still reject preparation.

After the readiness message, click the owned **Cueward #31 human input** window and its start button. In English input mode, type `aAaA` for 60 seconds, using physical Shift for uppercase, while moving the mouse. Do not switch Apps. At the end, release modifiers and press the finish button. The runner waits at most 180 seconds for start and 60 seconds for finish acknowledgement. The observer has a 900-second deadline, and receivers have a 960-second fallback lifetime derived from it. Cleanup stops only retained owned subprocess handles and deletes only verified owned snapshots; raw reports remain local, including errors.

Background actions are one text insertion, Cmd+A, replacement text, one native button click, one 80-by-40-point canvas drag and one scroll. Every action obtains a fresh PID-verified snapshot. Uncertain dispatch is recorded without replay. A 58-second input-start budget is checked both before the next action and after snapshot collection. Preparation, marked execution, command receipts and cleanup are separate.

A distinct receiver changes its title 120 ms after its App-level monitor sees the first annotated down, providing an independent identity-change interruption. This observation does not override first-mouse acceptance or deliver events to a view. A pre-dispatch click against the active human receiver must be rejected by the unchanged production foreground guard. **Neither case is a target becoming foreground during an in-flight operation**, which remains unrun.

For effect/routing diagnosis without asking the operator to type:

```sh
python3 -B dev/computer-use/physical-overlap-probe.py \
  --cli /absolute/path/to/cueward --parent /private/local/evidence --idle-diagnostic
```

This mode creates no human receiver and cannot establish physical-overlap acceptance. `recorded` means evidence was collected, not that its checks passed.

## Evidence boundaries

- Keyboard text/events are collected only inside newly owned receiver windows. No foreign/global keyboard contents are read. The human receiver also has a read-only global **mouse-movement-only** monitor so physical movement outside its window is not silently omitted. Mouse coordinates and all raw desktop/process identities remain local.
- The observer exposes its `system_uptime_origin`; receiver uptime and command markers are correlated on that shared timebase. Foreground, all online displays' visible Spaces, receiver activation, lock state and actual sample gaps are checked. Nonhuman activation notifications can reject transitions between samples.
- With complete command intervals, a foreground or visible-Space change between a command's start and one second after its end fails, whatever the input counts say, because typing advances them for the whole round. A command that runs while another app is in front leaves the desktop check unverified, since the human window could not be displaced. The recording must reach one second past the last command.
- Final text alone is insufficient. Background content, complete down/up pairs, down **and up** modifier flags, event annotations, button count, object displacement/release and actual scroll offset are checked together. Human text, key-event contents, Shift typing, movement, complete logs and explicit finish/modifier release are separate checks. An annotation is not hardware authentication.
- Pointer correlation uses a 100 ms / 15 point observation window. Missing correlation remains `unverified`, not an automation-warp finding or a no-warp pass. Matching samples do not authenticate causality or exclude shorter transient movement.
- Each command interval must contain human typing and mouse observations. Whole-round activity does not substitute for per-command overlap, and an interval match is not simultaneous individual-event delivery.
- App-level up delivery and release of an accepted view gesture are distinct. A sender stopping with an up event does not prove a view accepted or released a drag.

## Recorded results

The [aggregate report](results/2026-10-04-physical-overlap.json) contains synthetic counts/effects only, on macOS 27.0.1 build 26A434, arm64, three displays.

| Case | Observed result | Scope / status |
| --- | --- | --- |
| Actual human round | 35 human key-downs, including 17 with Shift, and 364 owned-window mouse-move observations; no background marker/annotation mixed into human input. Background 45 down/up pairs, exact replacement, Cmd+A and one click effect. | These effects were observed, not complete #31 acceptance. |
| Desktop in human round | 1,577 samples over 30.536 seconds; maximum gap 59.979 ms; fixed human foreground, both receivers inactive, all three visible-Space mappings unchanged. | Passed at sampled resolution, not continuous/all-app proof. |
| Canvas in human round | 52 events posted and received by the App monitor; object unchanged, no view release. | Failed effect check. |
| Scroll in human round | No offset change; later observation found the text document and viewport both 300 points high. | Initial fixture was not scrollable. This is not evidence of a platform scroll rejection. |
| Pointer / command overlap | 271 of 396 changed sample steps lacked correlation to the original owned-window-only mouse log; some command intervals had no local mouse movement. | Unverified. Mouse movement outside the window explains a coverage gap, not authenticated causality. |
| Finish / interruption / foreground | Initial fixture closed automatically and did not record explicit modifier-release acknowledgement; its view-level interruption never triggered. Pre-dispatch foreground refusal did occur without an annotated human-window event. | Finish and accepted interrupted gesture remain unverified; foreground refusal is only pre-dispatch. |
| Corrected idle receiver | Separate fixed-height scroll content moves offset 0 to 160. Text, Cmd+A, pairs/modifiers and click effects remain correct. Canvas still has no callbacks/displacement. | Scroll setup corrected; no replacement human round was run. |
| Independent idle interruption | Production returns `partially_sent`, seven events, `window changed`; original receiver monitor sees down/up. Canvas never starts a gesture. Maximum desktop gap 25.176 ms, stationary isolation check passed. | Dispatch stop/up observed; accepted gesture release unverified. |

## Idle diagnostic after click preparation: 2026-11-07

This run used Cueward 0.6.0 from crates.io and the unchanged fixtures. Nobody typed during it, so it does not replace the human round. The [aggregate](results/2026-11-07-physical-overlap-idle.json) keeps the counts.

| Check | Observed result | Status |
| --- | --- | --- |
| Background round | Text, Cmd+A, replacement, one click, one drag and one scroll in a single run on an inactive Space. The canvas object moved exactly 80 by 40 points and released once. Scroll offset reached 160. | Passed |
| Controlled interruption | The window changed during a drag. `partially_sent` with 3 events. The receiver saw the title bar pair, then down, dragged and up. The canvas released once. | Passed |
| Desktop | Maximum sample gap 59.4 ms, with input attribution. | Passed at sampled resolution |

The canvas failures above came from the receiver's first-mouse policy. [Click preparation](background-first-click.md) makes the window key first, so the ordinary receiver now accepts the drag without any change to it.

The first evaluation of this run reported the interruption as failed. The check allowed exactly one press pair, and click preparation sends a second one on the title bar. The check now requires each press to be released before the next one and none to stay held, so a stuck button still fails.

Still not covered: physical typing during the run, a target that becomes foreground during dispatch, physical WebView overlap and installed third-party apps.

## Human round: 2026-11-07

A person typed for 60 seconds with Cueward 0.6.1 at a load average near 45. The [aggregate](results/2026-11-07-physical-overlap-human.json) keeps the counts.

| Check | Observed result | Status |
| --- | --- | --- |
| Background | Text, Cmd+A, replacement, click, drag and scroll in one run, with the person typing. Object moved 80 by 40 points. | Passed |
| Human input | 111 key presses and 111 characters. No background annotation in the human receiver. Modifiers released. | Passed |
| Controlled interruption and foreground refusal | Both as expected. | Passed |
| Desktop | Another process held the foreground for 3,984 ms and overlapped a read-only `window snapshot`. | Failed |
| Command overlap | Key presses overlapped all six actions. The mouse did not move during them, because typing used both hands. | Unverified |
| Pointer correlation | 38 of 136 changes had no matching observation. | Unverified |
| Target becoming foreground during dispatch | Not exercised. | Unverified |

The process that took the foreground outlived the snapshot by 2.6 seconds, and production window code does not create an application object, so it was not a Cueward helper. It ended before it could be named. `folio-e2e` was restarting in this period, which is not proof. The desktop check stays failed, because it counts any change during a command.

The first attempt ran out of time: ten commands took 28.0 seconds at high load, and a drag took 7.5 seconds against 2.6 when idle. The round is now 60 seconds.

The human check first failed on `content_matches_key_events`. McBopomofo types English with CapsLock on, so every key event reports an uppercase letter, while the text follows Shift. The check now accepts the case that physical Shift gives. A wrong letter or a wrong case still fails. The desktop check now uses the [command-scoped rule](task-acceptance.md#evidence-contract) when receipts and markers are complete.

The initial scrollable NSTextView was resized to its viewport after attachment/layout. Minimum-size-only attempts before and after attachment also failed. A separate ordinary fixed-height scroll document now preserves the scroll challenge independently of short-text replacement. The corrected idle result does **not** retroactively pass the initial human round.

In a separate local diagnostic source copy, changing only the canvas first-mouse policy to accept produced one down, 50 drags, one up and the expected 80-by-40-point displacement. The matched ordinary receiver produced zero view callbacks. This supports the first-mouse gate explanation for this receiver; it is not a production fix, ordinary-app acceptance, or a WebView root cause. The patch is not shipped. Apple's [NSView first-mouse documentation](https://developer.apple.com/documentation/appkit/nsview/acceptsfirstmouse%28for%3A%29) describes its default refusal and native controls that accept first mouse.

New global mouse-only observation, manual finish acknowledgement and the corrected scroll layout have not been exercised together in another human round. Mid-flight foreground activation, physical WebView overlap, real installed app coverage and an accepted interrupted drag remain outstanding. First-mouse support investigation belongs with #32; do not bypass it through receiver changes, activation, global input, double-click/replay or Space switching.

## Verification

Offline tests cover content/event contamination, key/modifier release, interval correlation, incomplete evidence, desktop transitions/lock/gaps, independent membership checks, no replay, deadlines, snapshot ownership and cleanup despite corrupt state. The observer protocol test checks its real uptime origin. These tests are not desktop or human acceptance. The corrected ordinary fixture and unchanged production CLI were additionally exercised in an idle desktop diagnostic.
