#!/usr/bin/env python3
"""Compare first-save panel creation contexts without forcing controls or replaying Save."""

import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import time

from desktop_observer import DesktopObserver, compile_observer, observation_checks
from observed_cli import ObservedCLI, panel_controls, sole_target
from task_acceptance.setup import write_json

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("first_save_baseline", HERE / "first-save-probe.py")
BASELINE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASELINE)


def move_owned_window(commands, window_id):
    spaces = commands.call("space", "list")
    choices = [s["id"] for d in spaces["displays"] for s in d["spaces"] if s["type"] == 0 and not s["is_visible"]]
    if not choices:
        raise RuntimeError("no existing inactive Space; no desktop was created")
    membership = commands.call("space", "window", "--id", window_id)
    token = membership.get("move_target")
    if not isinstance(token, str) or not token:
        raise RuntimeError("missing current move target; no move was requested")
    result = commands.call("space", "move-window", "--target", token, "--space", choices[0])
    if result.get("status") != "confirmed" or result.get("foreground_changed") or result.get("visible_spaces_changed"):
        raise RuntimeError("background Space move did not preserve observed endpoints")
    return str(choices[0])


def observe_panel(commands, pid):
    deadline = time.monotonic() + 3
    incomplete = False
    while True:
        nodes, partial = commands.explore(pid)
        incomplete |= partial
        controls = panel_controls(nodes)
        if controls or time.monotonic() >= deadline:
            return controls, incomplete, nodes
        time.sleep(0.05)


def describe_controls(controls):
    return [{key: node.get(key) for key in ("role", "enabled", "identifier", "receiver_pid", "ref")}
            | {"has_target": bool(node.get("target"))} for node in controls]


def verify_window_spaces(commands, window_id, state_path, report):
    state = json.loads(state_path.read_text())
    panel_id = state.get("panel_window_id", 0)
    report["document_membership"] = commands.call("space", "window", "--id", window_id)
    if type(panel_id) is not int or panel_id <= 0:
        raise RuntimeError("receiver has no current attached Save panel identity")
    report["panel_membership"] = commands.call("space", "window", "--id", panel_id)
    if "target_space" in report:
        target = int(report["target_space"])
        if any(record.get("space_ids") != [target] for record in
               (report["document_membership"], report["panel_membership"])):
            raise RuntimeError("document/panel inactive Space membership is not confirmed")


def exercise(commands, pid, window_id, directory, context, report):
    if context == "inactive":
        report["target_space"] = move_owned_window(commands, window_id)
    nodes, incomplete = commands.explore(pid)
    report["editor_observation"] = nodes
    edited = commands.call("app", "set-value", "--target", sole_target(nodes, "draft-editor"), "--value", BASELINE.CONTENT)
    if edited.get("status") != "confirmed" or edited.get("foreground_changed"):
        raise RuntimeError("background document edit was not confirmed")
    report["edit_confirmed"] = True
    nodes, _ = commands.explore(pid)
    requested = commands.call("app", "press", "--target", sole_target(nodes, "save-document"))
    if requested.get("foreground_changed"):
        raise RuntimeError("document Save request changed foreground endpoints")
    controls, partial, panel_nodes = observe_panel(commands, pid)
    report["panel_traversal_incomplete"] = incomplete or partial
    report["controls_before_move"] = describe_controls(controls)
    if context == "warm":
        report["target_space"] = move_owned_window(commands, window_id)
        # Reinspect every current root; no panel target is reused across a Space move.
        controls, partial, panel_nodes = observe_panel(commands, pid)
        report["panel_traversal_incomplete"] |= partial
    report["controls_at_save"] = describe_controls(controls)
    report["panel_observation"] = panel_nodes
    report["file_created"] = False
    report["file_content_matches"] = False
    if len(controls) != 1:
        report["status"] = "save_control_missing_or_ambiguous"
        return
    if controls[0].get("enabled") is not True or not controls[0].get("target"):
        report["status"] = "save_control_disabled_or_unavailable"
        return
    verify_window_spaces(commands, window_id, directory / "receiver.json", report)
    report["panel_save_dispatch_count"] = 1
    saved = commands.call("app", "press", "--target", controls[0]["target"])
    if saved.get("foreground_changed"):
        raise RuntimeError("panel Save action changed foreground endpoints")
    artifact = directory / "artifact.txt"
    deadline = time.monotonic() + 5
    while not artifact.exists() and time.monotonic() < deadline:
        time.sleep(0.05)
    report["file_created"] = artifact.is_file()
    if report["file_created"]:
        actual = artifact.read_bytes()
        report["file_content_matches"] = actual == BASELINE.CONTENT.encode("utf-8")
        report["artifact_utf8"] = actual.decode("utf-8")
    report["status"] = "artifact_verified" if report["file_content_matches"] else "artifact_not_created_or_mismatched"


def launch_fixture(directory):
    source = directory / "DocumentFixture.swift"
    source.write_text((HERE / "document-observer.swift").read_text() + "\n" + (HERE / "document-fixture.swift").read_text())
    fixture = directory / "DocumentFixture"
    subprocess.run(["swiftc", "-module-cache-path", str(directory / "module-cache"), str(source), "-o", str(fixture)],
                   capture_output=True, check=True, timeout=90)
    return subprocess.Popen([str(fixture), str(directory / "receiver.json"), str(directory)], start_new_session=True)


def observe_exercise(cli, context, output, directory, binary, receiver, report):
    state_path = directory / "receiver.json"
    BASELINE.wait_file(state_path)
    state = json.loads(state_path.read_text())
    if state.get("active") is not False:
        raise RuntimeError("new receiver is not in the background; no task was begun")
    report["receiver_pids"] = [receiver.pid]
    with DesktopObserver(binary, [receiver.pid], output.with_suffix(".trace.jsonl"), duration_ms=180_000) as observer:
        commands = ObservedCLI(cli, observer)
        start = observer.mark("execution-start")
        try:
            exercise(commands, receiver.pid, state["window_id"], directory, context, report)
        except (OSError, ValueError, RuntimeError) as error:
            report.update(status="diagnostic_error", error=str(error))
        finally:
            report["receipts"] = commands.receipts
            end = observer.mark("execution-end")
            report["execution"] = observer.execution(start, end)
            report["observation_check"] = observation_checks(report["execution"], [receiver.pid])
            report["receiver_final"] = json.loads(state_path.read_text())


def probe(cli, context, output):
    # Reserve only this new report. No previous evidence is overwritten.
    with output.open("x", encoding="utf-8") as file:
        file.write('{"status":"preparing"}\n')
    report = {"kind": "diagnostic", "context": context, "status": "preparing", "panel_save_dispatch_count": 0,
              "limitations": "Synthetic NSDocument context comparison, not fresh-agent/real-app acceptance. No uncertain action replay. Cross-app reading is a separate task."}
    receiver = None
    try:
        with tempfile.TemporaryDirectory(prefix="cueward-first-save-context-") as temporary:
            try:
                directory = Path(temporary)
                binary = compile_observer(directory)
                receiver = launch_fixture(directory)
                observe_exercise(cli, context, output, directory, binary, receiver, report)
            finally:
                # Stop the writer before removing its destination directory.
                stop_receiver(receiver)
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        report.update(status="diagnostic_error", error=str(error))
    finally:
        write_json(output, report)
    return report


def stop_receiver(receiver):
    if receiver and receiver.poll() is None:
        receiver.terminate()
        try:
            receiver.wait(timeout=5)
        except subprocess.TimeoutExpired:
            receiver.kill()
            receiver.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--context", choices=("visible", "inactive", "warm"), required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    session = subprocess.run(["swift", "-e", 'import CoreGraphics; guard let s = CGSessionCopyCurrentDictionary() as? [String:Any] else { exit(1) }; print(s["CGSSessionScreenIsLocked"] as? Bool == true ? "locked" : "unlocked")'],
                             capture_output=True, check=True, timeout=10)
    if session.stdout.decode().strip() != "unlocked":
        raise RuntimeError("desktop locked; no receiver or Save request was started")
    report = probe(args.cli.resolve(), args.context, args.output.resolve())
    print(json.dumps({key: report.get(key) for key in ("context", "status", "file_created", "file_content_matches", "observation_check", "error")}))
    return 0 if report.get("status") == "artifact_verified" and report.get("observation_check", {}).get("status") == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
