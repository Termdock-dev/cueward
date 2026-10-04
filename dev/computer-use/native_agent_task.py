"""Owned native task setup and independent evidence; never selects agent actions."""
import os
from pathlib import Path
import plistlib
import subprocess
import time
import uuid

from task_acceptance.evaluate import load_json, read_scoped
from task_acceptance.setup import write_json

HERE = Path(__file__).resolve().parent
TASKS = {"existing_document": "existing.txt", "calculation": "result.txt"}


def task_setup(run, task_id):
    """Validate a prepared run and reserve private metadata outside task workspaces."""
    run = Path(run).resolve()
    manifest = load_json(run / "manifest.json")
    goals = [g for g in manifest["tasks"] if g["task_id"] == task_id]
    if manifest.get("schema") != 1 or task_id not in TASKS or len(goals) != 1:
        raise ValueError("expected one supported native task in a prepared run")
    artifact = run / "work" / task_id / TASKS[task_id]
    if goals[0]["artifact"] != str(artifact):
        raise ValueError("task destination differs from prepared scope")
    for parent in (run / "work", artifact.parent, run / "operator"):
        if parent.is_symlink() or parent.exists() and not parent.is_dir():
            raise ValueError("native task parent must be an unlinked directory")
    # Reserve once: reruns require a fresh prepared run, not a repaired old result.
    output = run / "operator" / task_id
    output.mkdir(parents=True, exist_ok=False)
    if task_id == "calculation":
        # A blank named file avoids first-save coverage; never seed an answer.
        with artifact.open("x", encoding="utf-8") as file:
            file.write("")
    read_scoped(run, str(artifact.relative_to(run)))
    return manifest, goals[0], artifact, output


def compile_fixture(output):
    """Build a uniquely owned complete bundle with normal NSDocument file IO."""
    output = Path(output)
    source = output / "ExistingDocument.swift"
    source.write_text((HERE / "document-observer.swift").read_text() + "\n"
                      + (HERE / "existing-document-fixture.swift").read_text())
    bundle = output / "ExistingDocument.app"
    binary = bundle / "Contents/MacOS/ExistingDocument"
    binary.parent.mkdir(parents=True)
    subprocess.run(["swiftc", "-swift-version", "6", "-parse-as-library", "-warnings-as-errors", "-module-name", "OwnedDocument",
                    str(source), "-o", str(binary)], capture_output=True, check=True, timeout=90)
    (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps({
        "CFBundleExecutable": binary.name, "CFBundleIdentifier": "dev.cueward.OwnedDocument." + uuid.uuid4().hex,
        "CFBundlePackageType": "APPL", "CFBundleName": "Owned document editor",
        "NSPrincipalClass": "NSApplication", "LSUIElement": True,
        "CFBundleDocumentTypes": [{"CFBundleTypeName": "Plain text", "CFBundleTypeRole": "Editor",
                                   "LSHandlerRank": "None", "LSItemContentTypes": ["public.plain-text"],
                                   "NSDocumentClass": "OwnedDocument.ExistingDocument"}],
    }))
    return binary


def launch_receiver(binary, state, artifact, lifetime=900):
    """Return the owned process handle; configuration does not open user files."""
    with Path(state).with_suffix(".stdout.log").open("x") as stdout, \
            Path(state).with_suffix(".stderr.log").open("x") as stderr:
        return subprocess.Popen([str(binary)], start_new_session=True, stdout=stdout, stderr=stderr,
                                env=os.environ | {"CUEWARD_STATE": str(state), "CUEWARD_FILE": str(artifact),
                                                  "CUEWARD_LIFETIME": str(lifetime)})


def receiver_state(path, process, timeout=5, require_inactive=True):
    """Read an acknowledged owned identity, never treating a PID-only file as ownership."""
    deadline = time.monotonic() + timeout
    while process.poll() is None and time.monotonic() < deadline:
        if Path(path).exists():
            state = load_json(path)
            if state.get("pid") != process.pid or type(state.get("window_id")) is not int or state["window_id"] <= 0:
                raise RuntimeError("receiver state identity differs from owned handle")
            if require_inactive and (state.get("active") is not False or state.get("activations") != 0):
                raise RuntimeError("owned receiver is active; no task actions are permitted")
            return state
        time.sleep(0.03)
    raise RuntimeError("owned receiver did not acknowledge readiness")


def stop_receiver(process):
    """Stop only the retained child handle, even if its state is missing or corrupted."""
    if process and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def unregister_fixture(binary):
    """Remove only the uniquely built owned bundle's LaunchServices registration."""
    binary = Path(binary)
    bundle = binary.parents[2]
    if bundle.name != "ExistingDocument.app" or binary.relative_to(bundle).parts != ("Contents", "MacOS", "ExistingDocument"):
        raise ValueError("not an owned native-task fixture bundle")
    registry = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
    result = subprocess.run([registry, "-u", str(bundle)], capture_output=True, text=True, timeout=20)
    return result.returncode


def independent_evidence(session, state, manifest, task_id, pid, artifact):
    """Bind independently read receiver state without replacing CLI/trace results."""
    if state.get("pid") != pid or session.get("receiver_pids") != [pid]:
        raise ValueError("receiver and actual session identity differ")
    if session.get("run_id") != manifest["run_id"] or session.get("task_id") != task_id:
        raise ValueError("session does not belong to this prepared task")
    result = {k: v for k, v in session.items() if k != "records"}
    result["observer"] = {"source": "receiver_observer", "run_id": manifest["run_id"],
                          "task_id": task_id, "pid": pid,
                          **{k: state.get(k) for k in ("file", "contents", "read_requests", "save_requests",
                                                       "data_requests", "error", "panel_window_id")}}
    result["coverage"] = {"document_opened_by_agent": state.get("file") == str(artifact)
                          and type(state.get("read_requests")) is int and state["read_requests"] > 0,
                          "calculation_uses_document_app": task_id == "calculation",
                          "first_save_panel": False, "native_file_dialog": False}
    return result


def merge_evidence(run, task):
    """Keep all eight tasks in the evaluator denominator and preserve other attempts."""
    path = Path(run) / "native-agent-evidence.json"
    evidence = load_json(path) if path.exists() else {"schema": 1, "run_id": task["run_id"],
                                                    "kind": "agent", "tasks": {}}
    if evidence.get("run_id") != task["run_id"] or evidence.get("kind") != "agent":
        raise ValueError("aggregate belongs to another run or evidence kind")
    if task["task_id"] in evidence["tasks"]:
        raise ValueError("existing task evidence must not be overwritten")
    evidence["tasks"][task["task_id"]] = task
    write_json(path, evidence)
    return path
