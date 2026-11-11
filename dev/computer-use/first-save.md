# Native new-document first-save probe

Prepared for issue [#33](https://github.com/Termdock-dev/cueward/issues/33), 2026-10-01.

```sh
python3 dev/computer-use/first-save-probe.py --cli /absolute/path/to/cueward --inactive-space --output first-save.json
```

The runner creates a disposable native NSDocument app, observes its current AX roots, edits its text area, reobserves the Save command and opens the standard system save panel. It uses generic `app inspect`, `app set-value`, `app press`, Space observation/movement and background file-open commands. No product app adapter or fixed AX path is added. The fixture supplies a disposable output directory and filename through the standard document save-panel preparation callback; it does not change enabled state, activate the app or bypass the panel by writing the artifact directly.

The inactive-Space variant moves only its own newly created window into an existing inactive user desktop. No desktop is created or switched. Each action target comes from fresh observation. A disabled, missing or ambiguous panel Save control is recorded as unavailable, with enabled/target evidence and traversal completeness; the action is not forced or resent. The independent completion check requires the file's exact UTF-8 bytes to match the synthetic document text. A second disposable application receives that file through `app open`, and its own reported contents must match. A successful workspace callback alone does not complete the task.

After requesting Save once, the runner polls fresh panel observations until a control is found or a three-second observation deadline expires; each CLI call retains its own timeout. A disabled or ambiguous control ends observation without another Save request. A missing Space move target is rejected before dispatch. Completion also requires the document to be inactive with zero recorded activations/foreground changes, the reader to have zero recorded activations, and the open result to report an inactive reader and unchanged foreground endpoints. Positive signals produce `background_interference`; missing or malformed background evidence produces `background_evidence_incomplete`. Matching file contents cannot override either failure.

The document fixture samples its own activation and foreground changes every 10 ms. Its observer is explicitly registered in the default, modal-panel and event-tracking run-loop modes, so entering the native Save panel does not remove the timer from the active mode. This still does not provide continuous visible-Space sampling and cannot establish complete foreground isolation. Full #33 acceptance also requires an ordinary third-party/native document app run and continuous Space evidence. A synthetic NSDocument result supplies a regression case, not an all-app compatibility guarantee. Additional timing cases, such as a panel opened before moving the document window, remain to be compared after the first controlled run.

If the panel Save request returns but the artifact does not appear within the five-second file wait, the runner returns `artifact_not_created` with the observed Save controls, `file_created: false`, error and background evidence. It does not open the reader or repeat Save. The CLI writes this captured report to the requested output path after fixture cleanup; it no longer loses the report by propagating the file-wait exception.

The runner checks for an unlocked session before starting. It terminates only its own document/reader processes and deletes its temporary artifacts; no existing document is overwritten or user app instance closed. Swift compilation and Python syntax checks passed. Desktop execution remains **unverified** because macOS was locked during preparation. The existing first-save limitation remains open; this branch contains no product workaround or completed-task claim.

Run deterministic orchestration regressions without desktop access:

```sh
python3 -B -m unittest discover -s dev/computer-use -p test_first_save_probe.py -v
python3 -B -m unittest discover -s dev/computer-use -p test_document_observer.py -v
```

The orchestration tests use disposable JSON/files and mocked CLI/process responses to check interference rejection, required evidence, delayed/disabled/ambiguous controls, observation deadlines, missing move targets and missing-artifact report persistence through the real entry point. The observer regression compiles the same Swift observer used by the document fixture and runs real Foundation run loops in default, modal-panel and event-tracking modes, without opening a GUI. It checks that callbacks continue in all three modes. These checks do not validate an actual unlocked Save panel, macOS input delivery or first-save compatibility.

## Bounded subtree continuation

The 2026-11-12 source change is an unreleased diagnostic improvement for #33, not a fix for disabled Save controls or a completed desktop acceptance run. Both this runner and the [context comparison](first-save-context.md) use [shared AX exploration](ax_exploration.py). No production Rust command, native target or input guard changes.

Each exploration starts with fresh app roots and the existing deep window reads. When a returned node has unobserved children, shallow reads of that observed ref expose siblings that a large earlier descendant hid. The runner never invents child paths from a count. Nodes merge by ref, not label, so equal Save labels stay ambiguous and repeated observations do not become duplicate controls.

The default budget is 32 CLI reads including initial/final root discovery, 4,096 retained nodes and eight seconds checked between reads. Each native subtree request still has the 500-node bound and the product's 12-level path limit. An in-flight call retains its transport timeout; eight seconds is not a hard cancellation deadline. A parent with more children than one shallow request can expose may remain incomplete. Budget exhaustion and unavailable branches remain incomplete, not an empty or fully inspected panel.

The public app instance, repeated node observations and final root list must remain consistent. Opaque targets are excluded from comparisons and never decoded or rewritten. A visible change or transport error stops the exploration without replay. If the time budget prevents the final root check, observations are retained without action targets. Public previews cannot authenticate identical replacement nodes or make a multi-call observation atomic; native action-target revalidation remains required.

Offline regressions exercise both real Python entry points against the mocked native CLI boundary, including Save beyond 500 nodes, changed app/root observations, distinct equal labels, disabled controls, deduplication and read/node/time limits:

```sh
python3 -B -m unittest discover -s dev/computer-use -p test_ax_exploration.py -v
```

After the operator unlocked the desktop, one new inactive-first context diagnostic [retained 1,028 nodes and observed disabled Save](results/2026-11-12-first-save-traversal.json). Traversal stayed incomplete. The document received one Save request, but its disabled panel Save received none and no artifact was created. Owned receiver/observer processes and the temporary directory were absent after cleanup. This is a new diagnostic result, not a completed first-save task; historical results are unchanged.
