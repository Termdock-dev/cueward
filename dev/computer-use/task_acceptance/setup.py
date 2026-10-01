"""Create fresh synthetic workspaces without launching or controlling any app."""

import hashlib
import json
from pathlib import Path
import tempfile
import uuid

CONTENT = "Cueward first-save artifact αβγ\n拋棄式文件驗收\n"
ORIGINAL = "Synthetic document αβγ\n待修改的一行\n"
EDITED = "Synthetic document αβγ\n已完成修改\n"
TASKS = {
    "new_document": ("native_ax", "Create and save the supplied Unicode text once in a new native document on an inactive Space."),
    "existing_document": ("native_ax", "Open the disposable copy, replace 待修改的一行 with 已完成修改, and save."),
    "cross_app": ("native_ax", "Open the new_document artifact in a different fresh reader app and confirm its exact contents."),
    "window_dialog": ("native_ax", "In two fresh owned windows, edit target.txt to the requested text, open its save dialog, and save. Preserve bystander.txt."),
    "calculation": ("native_ax", "Use a fresh permitted calculator or document app to calculate 19.95 × 3 + 7.50 × 2 − 5.00; save the two-decimal answer plus newline."),
    "webview_first_click": ("webview_pointer", "Increment a fresh ordinary background WKWebView counter exactly once with one click."),
    "canvas_drag": ("webview_pointer", "Move the fresh canvas object by 80 pixels horizontally and 40 vertically, then release, with one drag."),
    "canvas_interrupted": ("webview_pointer", "In a separate fresh canvas receiver, start one drag; the operator interrupts it during execution. Verify release without replay."),
}


def write_json(path, value):
    """Atomically replace a JSON report in its selected output directory."""
    path = Path(path)
    temporary = path.with_name(f".{path.name}.{uuid.uuid4()}.tmp")
    try:
        with temporary.open("x", encoding="utf-8") as output:
            output.write(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def prepare(parent):
    """Return a unique workspace; resetting means preparing another workspace."""
    root = Path(tempfile.mkdtemp(prefix="cueward-acceptance-", dir=parent)).resolve()
    run_id = str(uuid.uuid4())
    seeds = root / "seeds"
    seeds.mkdir()
    (seeds / "original.txt").write_text(ORIGINAL, encoding="utf-8")
    files = {"seeds/original.txt": ORIGINAL}
    goals = []
    for task_id, (family, goal) in TASKS.items():
        folder = root / "work" / task_id
        folder.mkdir(parents=True)
        if task_id in ("existing_document", "window_dialog"):
            name = "existing.txt" if task_id == "existing_document" else "target.txt"
            files[f"work/{task_id}/{name}"] = ORIGINAL
            files[f"work/{task_id}/bystander.txt"] = "Preserve this synthetic file.\n"
        destination = folder / "result.txt"
        if task_id == "existing_document":
            destination = folder / "existing.txt"
        elif task_id == "window_dialog":
            destination = folder / "target.txt"
        elif task_id == "cross_app":
            destination = root / "work/new_document/result.txt"
        goals.append({"task_id": task_id, "family": family, "goal": goal,
                      "workspace": str(folder), "artifact": str(destination),
                      "content": EDITED if task_id in ("existing_document", "window_dialog") else CONTENT if task_id in ("new_document", "cross_app") else None})
    for relative, content in files.items():
        (root / relative).write_text(content, encoding="utf-8")
    baseline = {p: hashlib.sha256(c.encode("utf-8")).hexdigest() for p, c in files.items()}
    manifest = {"schema": 1, "run_id": run_id, "tasks": goals, "baseline": baseline}
    write_json(root / "manifest.json", manifest)
    write_json(root / "agent-goals.json", {
        "run_id": run_id, "tasks": goals,
        "scope": "Only newly owned receiver PIDs/windows and this run's work directory; the operator supplies the permitted app instances. Never reuse targets from a previous run.",
    })
    write_json(root / "evidence-template.json", {
        "schema": 1, "run_id": run_id, "kind": "agent", "tasks": {},
    })
    return root
