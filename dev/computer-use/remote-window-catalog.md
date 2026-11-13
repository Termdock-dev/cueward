# Remote Save-panel window catalog diagnostic

Recorded 2026-11-13 on macOS 27.0.1 (26A434), arm64. This follows [#33](https://github.com/Termdock-dev/cueward/issues/33) and isolates a lookup defect in [#104](https://github.com/Termdock-dev/cueward/issues/104). The [sanitized aggregate](results/2026-11-13-remote-window-catalog.json) separates each result from its desktop-observation limits. Raw targets, desktop records, snapshots, process IDs and temporary paths remain private.

## Public getter comparison

Two fresh owned NSDocument receivers used the same instrumented fixture executable. The private diagnostic copy adds a weak panel reference and samples public getters at most every 250 ms. It does not replace the delegate, call validation, set focus, enable controls or press panel Save. This is an instrumented synthetic diagnostic, not unchanged-app or third-party acceptance.

| Observation | First panel created inactive | First panel created visible |
| --- | --- | --- |
| Save | Disabled, no target | Enabled, target present |
| Filename | `artifact.txt` | `artifact.txt` |
| Directory equals owned writable destination | Yes | Yes |
| Panel/document native key state | Both false | Both false |
| Panel on active Space | No | Yes |
| Document Save requests / panel Save dispatches | 1 / 0 | 1 / 0 |
| Artifact created | No | No |
| Sampled desktop check | Passed; max gap 34.85 ms | Passed; max gap 53.15 ms |

Lack of a native key window cannot by itself explain disabled Save: the visible control is enabled without one. The host's filename and directory getters are populated in both cases. These traces cover the document receiver and desktop scene, not complete remote-service activation states. This does not prove remote-service validation or filesystem access, identify the disabled-control cause, or prove that moving between Spaces is a valid repair.

## Native filename input is not a validation result

Separate fresh inactive and visible receivers received one ordinary filename-field click, Command+A and `type-text`. The field came from fresh shallow AX observation; coordinates used the current snapshot frame and image scale. The click targeted the host sheet's native ID. `input-status` reported dispatch-ready and delivery returned `sent_unverified`, with `pointer_preparation: no_safe_point`. The filename remained `artifact.txt` in both contexts. No Tab or panel Save was sent after this mismatch.

The inactive trace exceeded the 100 ms bound (150.23 ms); the visible trace failed for unattributed pointer movement. Neither is an isolation pass. These attempts cannot establish that native filename validation ran, even on the visible Space, and cannot explain inactive-first Save.

Preliminary attempts are retained in the aggregate rather than substituted for successful controls. One controller tuple-unpack error stopped before native input. Two visible attempts stopped after uncertain document AXPress results; the second independently observed one document request and a panel. A later attempt confirmed its single request normally. No uncertain Save was replayed. A passive attempt obtained no uniquely actionable editor from shared exploration; no edit or Save followed it.

## Two native windows, two catalog results

A separate read-only AX observer resolved the sheet's native ID and required its `AXWindow` owner to be `CFEqual` to the owned document. It then read only two descendant levels. The host-exposed content group and filename field pointed to a second native window; that window's complete-catalog owner matched the foreign receiver observed inside this same attached sheet. No service was selected by title or terminated.

In one fresh visible case, the same process obtained these results:

| Observed native window | Complete catalog, exact ID rows | Single-window query, exact ID rows |
| --- | --- | --- |
| Document | 1 | 1 |
| Host Save sheet | 1 | 1 |
| Remote content | 1 | 0 |

The complete remote-content row included its current owner, title and bounds. A separate pre-fix `space window` attempt for the independently bound content ID failed with `window changed; observe again`, before any native input. Its trace had a 997.81 ms gap. The passive catalog contrast had a 152.87 ms gap. These structural observations are not continuous desktop-isolation evidence.

The [read-only contrast](window-catalog-contrast.swift) reproduces the catalog comparison for up to four unique, currently observed owned window IDs:

```sh
swift dev/computer-use/window-catalog-contrast.swift <owned-window-id>
```

It prints counts, availability and identity equality, not titles, bounds, file contents or unrelated windows. Obtaining a native ID does not authorize operating that window; the caller must independently establish its owned document/sheet context. The probe does not perform that binding, activate an app, move a window or send input.

Apple documents [`optionAll`](https://developer.apple.com/documentation/coregraphics/cgwindowlistoption/optionall) as including onscreen and offscreen windows with `kCGNullWindowID`. A missing row from the narrower query does not erase the separately observed complete-catalog row. This is an observed platform/query difference, not a claim that every remote window behaves this way.

## Lookup repair and remaining boundary

The unreleased repair keeps the normal single-window query. Only when it contains no row for the requested ID does it read the complete live catalog. Both the AX/input helper and Space helper require exactly one row and then apply their unchanged identity guards. A conflicting or duplicate single-window row is rejected, not replaced by the complete catalog; a missing, duplicate or changed complete-catalog row is also rejected. No cached row, opaque-target rewriting, auto-retargeting or global input is added.

The deterministic regressions run the actual Swift catalog guard and Objective-C Space helper against OS-boundary replacements. Both fail before the repair with `remote window absent from individual lookup was rejected` and pass afterward. Negative cases retain PID, title, frame, offscreen, missing/unavailable and duplicate-row rejection. This is lookup verification, not remote-input acceptance.

The initial post-fix attempt stopped at locked-session preflight, before launching any receiver or observer. After the operator unlocked, a new owned receiver reproduced the remote-content row in the full catalog and its absence in both single-window query variants. With the repaired CLI:

- `space window` returned the observed content ID without the false identity-change error. Its membership was `[]`, so no move target was issued. This is unavailable independent membership, not proof of a movable content Space or equality with its parent sheet.
- A fresh remote-content snapshot succeeded. Read-only `input-status` returned pointer dispatch-ready, while keyboard routing remained unavailable because this was not the service's verified keyboard window. No input was redirected or sent, and application acceptance remained unverified.
- The document received one Save-panel request; panel Save and native input dispatches were both zero. No artifact was created. The receiver ended inactive with zero recorded activations.
- The sampled document/desktop check passed within its 100 ms bound, with a maximum gap of 91.23 ms. This is not complete service-activation or input-isolation proof. Owned receiver/observer processes and their context directory were absent after cleanup.

This completes the live lookup check for #104, not inactive-first saving. The original disabled Save, third-party new-document creation and cross-app artifact acceptance remain unresolved. #33 and #36 stay open.

The CI-matching Rust 1.88.0 locked release build and full serial workspace suite passed: 963 tests passed and 38 were ignored. The changed-Rust formatting script also passed. An earlier Rust 1.99.0 full run failed once in an unchanged files-info fixture setup; the isolated rerun and the full 1.88.0 run passed. Its cause is unconfirmed, and no unrelated test was changed or suppressed. The whole-workspace stable formatter reports existing differences outside the changed Rust file; this patch does not reformat them.
