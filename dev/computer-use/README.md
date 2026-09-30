# Native new-document first-save probe

Prepared for issue [#33](https://github.com/Termdock-dev/cueward/issues/33), 2026-10-01.

```sh
python3 dev/computer-use/first-save-probe.py --cli /absolute/path/to/cueward --inactive-space --output first-save.json
```

The runner creates a disposable native NSDocument app, observes its current AX roots, edits its text area, reobserves the Save command and opens the standard system save panel. It uses generic `app inspect`, `app set-value`, `app press`, Space observation/movement and background file-open commands. No product app adapter or fixed AX path is added. The fixture supplies a disposable output directory and filename through the standard document save-panel preparation callback; it does not change enabled state, activate the app or bypass the panel by writing the artifact directly.

The inactive-Space variant moves only its own newly created window into an existing inactive user desktop. No desktop is created or switched. Each action target comes from fresh observation. A disabled, missing or ambiguous panel Save control is recorded as unavailable, with enabled/target evidence and traversal completeness; the action is not forced or resent. The independent completion check requires the file's exact UTF-8 bytes to match the synthetic document text. A second disposable application receives that file through `app open`, and its own reported contents must match. A successful workspace callback alone does not complete the task.

The document fixture samples its own activation and foreground changes every 10 ms. This does not provide continuous visible-Space sampling and cannot establish complete foreground isolation. Full #33 acceptance also requires an ordinary third-party/native document app run and continuous Space evidence. A synthetic NSDocument result supplies a regression case, not an all-app compatibility guarantee. Additional timing cases, such as a panel opened before moving the document window, remain to be compared after the first controlled run.

The runner checks for an unlocked session before starting. It terminates only its own document/reader processes and deletes its temporary artifacts; no existing document is overwritten or user app instance closed. Swift compilation and Python syntax checks passed. Desktop execution remains **unverified** because macOS was locked during preparation. The existing first-save limitation remains open; this branch contains no product workaround or completed-task claim.
