"""Owned new-document and cross-app setup and evidence, without a scripted task."""
import json
from pathlib import Path
import plistlib
import subprocess
import uuid

from agent_session import AgentSession
from desktop_observer import DesktopObserver, compile_observer
from native_agent_task import launch_receiver, merge_evidence, receiver_state, session_evidence
from observed_cli import parse_cli_output
from task_acceptance.evaluate import evaluate, load_json
from task_acceptance.setup import write_json

HERE = Path(__file__).resolve().parent
DOCUMENT_TASKS = ("new_document", "cross_app")
FINAL_FIELDS = ("active", "activations", "window_id", "observation_sequence", "fresh_after_sequence")


def document_setup(run, task_id):
    """Validate the prepared task and reserve private metadata once."""
    run = Path(run).resolve()
    manifest = load_json(run / "manifest.json")
    goals = [g for g in manifest["tasks"] if g["task_id"] == task_id]
    artifact = run / "work/new_document/result.txt"
    if (manifest.get("schema") != 1 or task_id not in DOCUMENT_TASKS or len(goals) != 1
            or goals[0]["artifact"] != str(artifact)):
        raise ValueError("expected one prepared document task for the shared artifact")
    for parent in (run / "work", artifact.parent, run / "operator"):
        if parent.is_symlink() or parent.exists() and not parent.is_dir():
            raise ValueError("document task parent must be an unlinked directory")
    if task_id == "new_document" and (artifact.exists() or artifact.is_symlink()):
        raise ValueError("a first save needs a destination with no existing file")
    output = run / "operator" / task_id
    output.mkdir(parents=True, exist_ok=False)
    return manifest, goals[0], artifact, output


def compile_swift(output, name, fixture, flags, binary):
    source = output / (name + ".swift")
    source.write_text((HERE / "document-observer.swift").read_text() + "\n" + (HERE / fixture).read_text())
    compiled = subprocess.run(["swiftc", *flags, "-module-cache-path", str(output / "module-cache"),
                               str(source), "-o", str(binary)], capture_output=True, timeout=90)
    (output / "fixture-compiler.stdout.log").write_bytes(compiled.stdout)
    (output / "fixture-compiler.stderr.log").write_bytes(compiled.stderr)
    compiled.check_returncode()
    return binary


def compile_document_fixture(output):
    """Build a bare executable: inside a bundle, AppKit opens positional paths as documents."""
    return compile_swift(output, "NewDocument", "document-fixture.swift", ["-module-name", "NewDocument"],
                         output / "NewDocument")


def compile_reader(output):
    """Build a uniquely owned bundle whose only access to the artifact is reading."""
    bundle = output / "DocumentReader.app"
    binary = bundle / "Contents/MacOS/DocumentReader"
    binary.parent.mkdir(parents=True)
    compile_swift(output, "DocumentReader", "document-reader-fixture.swift",
                  ["-swift-version", "6", "-parse-as-library", "-warnings-as-errors", "-module-name", "OwnedReader"], binary)
    (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps({
        "CFBundleExecutable": binary.name, "CFBundleIdentifier": "dev.cueward.DocumentReader." + uuid.uuid4().hex,
        "CFBundlePackageType": "APPL", "CFBundleName": "Owned document reader",
        "NSPrincipalClass": "NSApplication", "LSUIElement": True,
    }))
    return binary


def launch_document(binary, state, artifact, lifetime=900):
    """Return the owned process handle; this fixture takes its destination as arguments."""
    with Path(state).with_suffix(".stdout.log").open("x") as stdout, \
            Path(state).with_suffix(".stderr.log").open("x") as stderr:
        return subprocess.Popen([str(binary), str(state), str(artifact.parent), artifact.name, str(lifetime)],
                                start_new_session=True, stdout=stdout, stderr=stderr)


def cli_json(cli, *arguments):
    """Run one setup CLI command before the Agent's broker exists."""
    result = subprocess.run([str(cli), *map(str, arguments)], capture_output=True, text=True, timeout=45)
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or "setup CLI command failed")
    return parse_cli_output(result.stdout.strip())


def move_to_inactive_space(cli, window_id, record):
    """Move only the owned window to an existing inactive Space; never create or switch one."""
    spaces = cli_json(cli, "space", "list")
    choices = [s["id"] for d in spaces["displays"] for s in d["spaces"] if s["type"] == 0 and not s["is_visible"]]
    if not choices:
        raise RuntimeError("no existing inactive Space; no desktop created")
    target = cli_json(cli, "space", "window", "--id", window_id).get("move_target")
    if not isinstance(target, str) or not target:
        raise RuntimeError("window has no observed move target; no move was requested")
    moved = cli_json(cli, "space", "move-window", "--target", target, "--space", choices[0])
    write_json(record, moved)
    if (moved.get("status") != "confirmed" or moved.get("foreground_changed") is not False
            or moved.get("visible_spaces_changed") is not False or moved.get("after_spaces") != [choices[0]]):
        raise RuntimeError("Space move did not preserve foreground and visible Spaces")
    return str(choices[0])


def prepare_receiver(task_id, cli, process, output):
    """Confirm an untouched receiver, then finish owned setup before the Agent starts."""
    initial = receiver_state(output / "receiver.json", process)
    if task_id == "cross_app":
        if initial.get("file") or initial.get("content") or initial.get("read_requests") != 0:
            raise RuntimeError("reader did not start with nothing opened")
        return initial, None
    if initial.get("contents") or initial.get("save_requests") != 0 or initial.get("file_created") is not False:
        raise RuntimeError("receiver did not start with an empty unsaved document")
    return initial, move_to_inactive_space(cli, initial["window_id"], output / "space-move.json")


def passed_producer(run):
    """Return this run's passed new_document PID from its evidence, or stop before any launch."""
    refused = "cross_app not started: new_document has no passed evidence in this run"
    try:
        evidence = load_json(Path(run) / "native-agent-evidence.json")
        report = evaluate(run, evidence)
    except (OSError, ValueError) as error:
        raise SystemExit(f"{refused} ({error}). Nothing was launched.") from error
    result = next(t for t in report["tasks"] if t["task_id"] == "new_document")
    if result["status"] != "passed":
        raise SystemExit(f"{refused} (status {result['status']}: {result.get('reason')}). Nothing was launched.")
    return evidence["tasks"]["new_document"]["observer"]["pid"]


def observed(final, pid, manifest, task_id, fields):
    """Keep only fields the receiver reported, so a missing one stays missing."""
    if final.get("pid") != pid:
        raise ValueError("receiver and actual session identity differ")
    return {"source": "receiver_observer", "run_id": manifest["run_id"], "task_id": task_id, "pid": pid,
            **{k: final[k] for k in (*fields, *FINAL_FIELDS) if k in final}}


def new_document_evidence(session, final, manifest, pid, artifact, target_space):
    """Record the attempt whether or not the first save produced an artifact."""
    result = session_evidence(session, manifest, "new_document", pid)
    result["target_space"] = target_space
    result["observer"] = observed(final, pid, manifest, "new_document",
                                  ("save_requests", "contents", "file_created", "panel_window_id"))
    requests = final.get("save_requests")
    result["coverage"] = {"inactive_space_setup": True, "first_save_panel": type(requests) is int and requests > 0}
    if not artifact.is_file():
        result["failure_category"] = "missing_artifact"
    return result


def reader_evidence(session, final, manifest, pid, producer_pid):
    """Bind the reader's own observation to the producer found in this run's evidence."""
    if type(producer_pid) is not int or producer_pid <= 0 or producer_pid == pid:
        raise ValueError("reader must be distinct from an observed producer")
    result = session_evidence(session, manifest, "cross_app", pid)
    result["producer_pid"] = producer_pid
    result["observer"] = observed(final, pid, manifest, "cross_app", ("file", "content", "read_requests", "error"))
    reads = final.get("read_requests")
    result["coverage"] = {"document_opened_by_agent": type(reads) is int and reads > 0}
    return result


def serve_agent(cli, process, initial, output, metadata, socket, observer_binary):
    with DesktopObserver(observer_binary, [process.pid], output / "session.trace.jsonl", duration_ms=900_000) as observer:
        session = AgentSession(cli, observer, process.pid, initial["window_id"], output / "session.json", metadata)
        session.serve_socket(socket, duration=800)
    # Start a new sequence baseline after the observer closes, then wait for a newer snapshot.
    final = receiver_state(output / "receiver.json", process, require_inactive=False,
                           fresh=True, expected_window_id=initial["window_id"])
    write_json(output / "receiver-final.json", final)
    return final


def run_document_task(cli, run, task_id, socket, *, write_report, record_failure, cleanup_receiver):
    """Share the entry runner's checkpoint, report and cleanup; the Agent does the task."""
    producer_pid = passed_producer(run) if task_id == "cross_app" else None
    manifest, goal, artifact, output = document_setup(run, task_id)
    process, bundle = None, None
    try:
        if task_id == "cross_app":
            fixture = bundle = compile_reader(output)
            launch = launch_receiver
        else:
            fixture, launch = compile_document_fixture(output), launch_document
        observer_binary = compile_observer(output)
        process = launch(fixture, output / "receiver.json", artifact)
        initial, target_space = prepare_receiver(task_id, cli, process, output)
        metadata = {"run_id": manifest["run_id"], "task_id": task_id, "attempted": True}
        write_json(output / "owned-scope.json", {**metadata, "pid": process.pid, "window_id": initial["window_id"],
                                                 "socket": str(socket), "goal": goal})
        final = serve_agent(cli, process, initial, output, metadata, socket, observer_binary)
        session = load_json(output / "session.json")
        evidence = (reader_evidence(session, final, manifest, process.pid, producer_pid) if task_id == "cross_app"
                    else new_document_evidence(session, final, manifest, process.pid, artifact, target_space))
        path = merge_evidence(run, evidence)
        print(json.dumps({"task_id": task_id, "report": str(write_report(run, path))}), flush=True)
    except BaseException as error:
        record_failure(run, output, manifest, task_id, process, error)
        if not (output / "session.json").exists():
            path = merge_evidence(run, {"run_id": manifest["run_id"], "task_id": task_id, "attempted": False,
                                       "prerequisite": "fixture_unavailable", "blocked_reason": str(error)})
            write_report(run, path)
        raise
    finally:
        cleanup_receiver(process, bundle, output, task_id)
