#!/usr/bin/env python3
"""Diagnose ordinary WKWebView background click/drag effects using the public CLI."""
import argparse
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent


def run(command):
    result = subprocess.run(command, capture_output=True, timeout=45)
    if result.returncode:
        raise RuntimeError(result.stderr.decode().strip())
    text = result.stdout.decode().strip()
    if text.startswith("<external "):
        text = text.split("\n", 1)[1].rsplit("\n</external>", 1)[0]
    return json.loads(text) if text else None


def read_state(path):
    return json.loads(path.read_text())


def wait_state(path):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if path.exists():
            state = read_state(path)
            if state["dom"].get("regions"):
                return state
        time.sleep(0.05)
    raise RuntimeError("WebView fixture failed to load")


def save_report(output, report):
    # Replace the checkpoint atomically so the last complete trial survives later errors.
    with tempfile.NamedTemporaryFile(mode="w", dir=output.parent, prefix=output.name + ".", delete=False) as checkpoint:
        path = Path(checkpoint.name)
        try:
            checkpoint.write(json.dumps(report, indent=2) + "\n")
            checkpoint.close()
            path.replace(output)
        finally:
            path.unlink(missing_ok=True)


def trial_command(shot, before, name):
    dom = before["dom"]
    def point(x, y):
        return [str(x * shot["image"]["scale_x"]),
                str((y + before["content_top"]) * shot["image"]["scale_y"])]
    if name != "canvas_drag":
        rect = dom["regions"]["button"]
        x, y = point(rect["x"] + rect["width"] / 2, rect["y"] + rect["height"] / 2)
        return ["window", "click", "--target", shot["input_target"], "--x", x, "--y", y, "--count", "1"]
    rect, obj = dom["regions"]["canvas"], dom["object"]
    x, y = point(rect["x"] + obj["x"], rect["y"] + obj["y"])
    to_x, to_y = point(rect["x"] + obj["x"] + 80, rect["y"] + obj["y"] + 40)
    return ["window", "drag", "--target", shot["input_target"], "--x", x, "--y", y,
            "--to-x", to_x, "--to-y", to_y, "--duration-ms", "500"]


def effects(before, after):
    dom = before["dom"]
    return {"click_effects": after["dom"]["clicks"] - dom["clicks"],
            "object_displacement": {axis: after["dom"]["object"][axis] - dom["object"][axis] for axis in ("x", "y")},
            "release_effects": after["dom"]["releases"] - dom["releases"],
            "dom_events": after["dom"]["events"][len(dom["events"]):], "target_active": after["active"],
            "target_activations": after["activations"] - before["activations"],
            "foreground_changes": after["foreground_changes"] - before["foreground_changes"],
            "pointer_different_samples": after["pointer_different_samples"] - before["pointer_different_samples"],
            "observation_samples": after["observations"] - before["observations"]}


def perform_trial(cli, window_id, state_path, images, name, blocked):
    trial = {"trial": name, "status": "observed"}
    before = None
    try:
        if blocked:
            trial["status"] = "skipped_previous_trial_error"
        else:
            before = read_state(state_path)
            if before["active"]:
                trial.update(status="skipped_foreground", target_active=True)
            else:
                shot = run([str(cli), "window", "snapshot", "--id", str(window_id)])
                images.add(Path(shot["screenshot"]["path"]))
                before = read_state(state_path)
                command = trial_command(shot, before, name)
                trial["status"] = "dispatch_failed"
                result = run([str(cli)] + command)
                trial.update(status="observed", dispatch_status=result["status"], posted_events=result["events_sent"])
                if result.get("interruption") or result["status"] == "partially_sent":
                    trial.update(status="dispatch_interrupted", interruption=result.get("interruption"))
                # CLI/helper completion is synchronous; this is additional receiver settling time.
                time.sleep(0.3)
                trial.update(effects(before, read_state(state_path)))
    except Exception as error:
        if trial["status"] == "observed":
            trial["status"] = "observation_failed"
        trial["error"] = str(error)
        if before is not None:
            try:
                trial.update(effects(before, read_state(state_path)))
            except Exception as observation_error:
                trial["observation_error"] = str(observation_error)
    return trial


def receiver_trials(cli, inactive_space, binary, directory, names, record):
    state_path = directory / (names[0] + ".json")
    receiver = subprocess.Popen([str(binary), str(state_path), str(HERE / "webview-fixture.html")], start_new_session=True)
    images, recorded = set(), set()
    try:
        state = wait_state(state_path)
        window_id = state["window_id"]
        if state["active"]:
            raise RuntimeError("fixture is foreground; refusing background trial")
        if inactive_space:
            spaces = run([str(cli), "space", "list"])
            choices = [s["id"] for d in spaces["displays"] for s in d["spaces"] if s["type"] == 0 and not s["is_visible"]]
            if not choices:
                raise RuntimeError("no existing inactive user Space; no desktop is created automatically")
            membership = run([str(cli), "space", "window", "--id", str(window_id)])
            target = membership.get("move_target")
            if not isinstance(target, str) or not target:
                raise RuntimeError("window has no observed move target; no move was requested")
            moved = run([str(cli), "space", "move-window", "--target", target, "--space", str(choices[0])])
            if moved["status"] != "confirmed" or moved["foreground_changed"] or moved["visible_spaces_changed"]:
                raise RuntimeError("inactive-Space move was not confirmed without endpoint changes")
        blocked = False
        for name in names:
            trial = perform_trial(cli, window_id, state_path, images, name, blocked)
            trial["receiver_pid"] = receiver.pid
            record(trial)
            recorded.add(name)
            blocked |= trial["status"] not in ("observed", "skipped_foreground")
    except Exception as error:
        for name in names:
            if name not in recorded:
                record({"trial": name, "receiver_pid": receiver.pid, "status": "setup_failed", "error": str(error)})
    finally:
        if receiver.poll() is None:
            os.killpg(receiver.pid, signal.SIGTERM)
        receiver.wait(timeout=5)
        for path in images:
            path.unlink(missing_ok=True)


def probe(cli, inactive_space, output):
    report = {"macos": platform.mac_ver()[0], "architecture": platform.machine(), "inactive_space": inactive_space,
              "observer_interval_ms": 10, "observation_limit": "timer sampling can miss transient changes; physical pointer movement is not attributed to the agent",
              "status": "running", "trials": []}
    save_report(output, report)
    def record(trial):
        report["trials"].append(trial)
        save_report(output, report)
    with tempfile.TemporaryDirectory(prefix="cueward-webview-") as temporary:
        directory = Path(temporary)
        binary = directory / "Receiver"
        try:
            subprocess.run(["swiftc", str(HERE / "webview-fixture.swift"), "-o", str(binary)], check=True, timeout=45)
            # Keep the two-click sequence, but give the independent drag its own fresh receiver.
            for names in (("first_single_click", "subsequent_single_click"), ("canvas_drag",)):
                try:
                    receiver_trials(cli, inactive_space, binary, directory, names, record)
                except Exception as error:
                    existing = {trial["trial"] for trial in report["trials"]}
                    for name in names:
                        if name not in existing:
                            record({"trial": name, "status": "setup_failed", "error": str(error)})
                    report.setdefault("receiver_errors", []).append(str(error))
                    save_report(output, report)
        except Exception as error:
            report["fatal_error"] = str(error)
    report["status"] = "recorded_with_errors" if (report.get("fatal_error") or report.get("receiver_errors")
        or any(trial["status"] != "observed" for trial in report["trials"])) else "recorded"
    save_report(output, report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--inactive-space", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    session = subprocess.run(["swift", "-e", 'import CoreGraphics; guard let s = CGSessionCopyCurrentDictionary() as? [String:Any] else { exit(1) }; print(s["CGSSessionScreenIsLocked"] as? Bool == true ? "locked" : "unlocked")'], capture_output=True, check=True, timeout=10)
    if session.stdout.decode().strip() != "unlocked":
        raise RuntimeError("desktop is locked; no fixture or input trial was started")
    report = probe(args.cli.resolve(), args.inactive_space, args.output.resolve())
    for trial in report["trials"]:
        print(trial["trial"], trial["status"], trial.get("click_effects"), trial.get("object_displacement"))
    return 1 if report["status"] == "recorded_with_errors" else 0


if __name__ == "__main__":
    raise SystemExit(main())
