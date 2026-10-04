"""Owned native task setup and independent evidence; never selects agent actions."""
import hashlib
import os
from pathlib import Path
import plistlib
import subprocess
import time
import uuid

from task_acceptance.evaluate import load_json, read_scoped
from task_acceptance.setup import write_json

HERE = Path(__file__).resolve().parent
TASKS = {"existing_document": "existing.txt", "calculation": "result.txt", "window_dialog": "target.txt"}


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
    if task_id in ("existing_document", "window_dialog"):
        files = [artifact] + ([artifact.with_name("bystander.txt")] if task_id == "window_dialog" else [])
        for file in files:
            relative = str(file.relative_to(run))
            actual = hashlib.sha256(read_scoped(run, relative)).hexdigest()
            if actual != manifest.get("baseline", {}).get(relative):
                raise ValueError("existing document differs from prepared baseline")
    # Reserve once: reruns require a fresh prepared run, not a repaired old result.
    output = run / "operator" / task_id
    output.mkdir(parents=True, exist_ok=False)
    if task_id == "calculation":
        # A blank named file avoids first-save coverage; never seed an answer.
        with artifact.open("x", encoding="utf-8") as file:
            file.write("")
    read_scoped(run, str(artifact.relative_to(run)))
    return manifest, goals[0], artifact, output


def compile_fixture(output, task_id="existing_document"):
    """Build a uniquely owned complete bundle with normal NSDocument file IO."""
    output = Path(output)
    dialog = task_id == "window_dialog"
    name, module, document = ("WindowDialog", "OwnedWindowDialog", "OwnedDialogDocument") if dialog else (
        "ExistingDocument", "OwnedDocument", "ExistingDocument")
    source = output / (name + ".swift")
    source.write_text((HERE / "document-observer.swift").read_text() + "\n"
                      + (HERE / ("window-dialog-fixture.swift" if dialog else "existing-document-fixture.swift")).read_text())
    bundle = output / (name + ".app")
    binary = bundle / "Contents/MacOS" / name
    binary.parent.mkdir(parents=True)
    compiled = subprocess.run(["swiftc", "-swift-version", "6", "-parse-as-library", "-warnings-as-errors", "-module-name", module,
                               str(source), "-o", str(binary)], capture_output=True, timeout=90)
    (output / "fixture-compiler.stdout.log").write_bytes(compiled.stdout)
    (output / "fixture-compiler.stderr.log").write_bytes(compiled.stderr)
    compiled.check_returncode()
    (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps({
        "CFBundleExecutable": binary.name, "CFBundleIdentifier": "dev.cueward." + name + "." + uuid.uuid4().hex,
        "CFBundlePackageType": "APPL", "CFBundleName": "Owned document editor",
        "NSPrincipalClass": "NSApplication", "LSUIElement": True,
        "CFBundleDocumentTypes": [{"CFBundleTypeName": "Plain text", "CFBundleTypeRole": "Editor",
                                   "LSHandlerRank": "None", "LSItemContentTypes": ["public.plain-text"],
                                   "NSDocumentClass": module + "." + document}],
    }))
    return binary


def launch_receiver(binary, state, artifact, lifetime=900, bystander=None):
    """Return the owned process handle; configuration does not open user files."""
    with Path(state).with_suffix(".stdout.log").open("x") as stdout, \
            Path(state).with_suffix(".stderr.log").open("x") as stderr:
        return subprocess.Popen([str(binary)], start_new_session=True, stdout=stdout, stderr=stderr,
                                env=os.environ | ({"CUEWARD_BYSTANDER": str(bystander)} if bystander else {})
                                | {"CUEWARD_STATE": str(state), "CUEWARD_FILE": str(artifact),
                                                  "CUEWARD_LIFETIME": str(lifetime)})


def receiver_state(path, process, timeout=5, require_inactive=True, *, fresh=False, expected_window_id=None):
    """Bind owned identity; a fresh read waits beyond a newly read sequence baseline."""
    if fresh and (type(expected_window_id) is not int or expected_window_id <= 0):
        raise ValueError("fresh reads require the owned window identity")
    deadline = time.monotonic() + timeout
    baseline = None
    while process.poll() is None and time.monotonic() < deadline:
        if Path(path).exists():
            state = load_json(path)
            if (state.get("pid") != process.pid or type(state.get("window_id")) is not int or state["window_id"] <= 0
                    or expected_window_id is not None and state["window_id"] != expected_window_id):
                raise RuntimeError("receiver state identity differs from owned handle")
            if require_inactive and (state.get("active") is not False or state.get("activations") != 0):
                raise RuntimeError("owned receiver is active; no task actions are permitted")
            if not fresh:
                return state
            sequence = state.get("observation_sequence")
            if type(sequence) is not int or sequence <= 0 or baseline is not None and sequence < baseline:
                raise RuntimeError("receiver observation sequence is invalid or regressed")
            if baseline is None:
                baseline = sequence
            elif sequence > baseline:
                return state | {"fresh_after_sequence": baseline}
        time.sleep(0.03)
    raise RuntimeError("owned receiver did not acknowledge a fresh observation" if fresh
                       else "owned receiver did not acknowledge readiness")


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
    name = binary.name
    if name not in ("ExistingDocument", "WindowDialog") or bundle.name != name + ".app" or binary.relative_to(bundle).parts != ("Contents", "MacOS", name):
        raise ValueError("not an owned native-task fixture bundle")
    registry = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
    result = subprocess.run([registry, "-u", str(bundle)], capture_output=True, text=True, timeout=20)
    return result.returncode


def session_evidence(session, manifest, task_id, pid):
    """Bind actual session metadata without inventing receiver observations."""
    if session.get("receiver_pids") != [pid]:
        raise ValueError("actual session identity differs from owned handle")
    if session.get("run_id") != manifest["run_id"] or session.get("task_id") != task_id:
        raise ValueError("session does not belong to this prepared task")
    return {k: v for k, v in session.items() if k not in ("records", "observer", "coverage")}


def independent_evidence(session, state, manifest, task_id, pid, artifact):
    """Require observed owned-file read/save/write, then retain actual CLI/trace results."""
    result = session_evidence(session, manifest, task_id, pid)
    if state.get("pid") != pid:
        raise ValueError("receiver and actual session identity differ")
    opened = state.get("file") == str(artifact) and type(state.get("read_requests")) is int and state["read_requests"] > 0
    saved = all(type(state.get(k)) is int and state[k] > 0 for k in ("save_requests", "data_requests"))
    if not opened or not saved or state.get("error") != "" or state.get("document_edited") is not False:
        raise ValueError("owned document open/save/write observation is incomplete or failed")
    result["observer"] = {"source": "receiver_observer", "run_id": manifest["run_id"],
                          "task_id": task_id, "pid": pid,
                          **{k: state.get(k) for k in ("file", "contents", "read_requests", "save_requests",
                                                       "data_requests", "error", "document_edited", "panel_window_id",
                                                       "active", "activations", "window_id", "observation_sequence", "fresh_after_sequence")}}
    result["coverage"] = {"document_opened_by_agent": opened,
                          "calculation_uses_document_app": task_id == "calculation" and opened,
                          "first_save_panel": False, "native_file_dialog": False}
    return result


def incomplete_session_evidence(path, manifest, task_id, pid):
    """Preserve a started attempt without publishing a passing partial receiver."""
    result = session_evidence(load_json(path), manifest, task_id, pid)
    if result.get("attempted") is not True:
        raise ValueError("session has no acknowledged attempt metadata")
    result["failure_category"] = "partial_observation"
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
