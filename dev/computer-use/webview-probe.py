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


def probe(cli, inactive_space):
    with tempfile.TemporaryDirectory(prefix="cueward-webview-") as temporary:
        directory = Path(temporary)
        binary = directory / "Receiver"
        subprocess.run(["swiftc", str(HERE / "webview-fixture.swift"), "-o", str(binary)], check=True, timeout=45)
        state_path = directory / "state.json"
        receiver = subprocess.Popen([str(binary), str(state_path), str(HERE / "webview-fixture.html")], start_new_session=True)
        images = set()
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
                moved = run([str(cli), "space", "move-window", "--target", membership["move_target"], "--space", str(choices[0])])
                if moved["status"] != "confirmed" or moved["foreground_changed"] or moved["visible_spaces_changed"]:
                    raise RuntimeError("inactive-Space move was not confirmed without endpoint changes")
            def snapshot():
                shot = run([str(cli), "window", "snapshot", "--id", str(window_id)])
                images.add(Path(shot["screenshot"]["path"]))
                return shot
            def point(shot, x, y):
                current = read_state(state_path)
                return [str(x * shot["image"]["scale_x"]), str((y + current["content_top"]) * shot["image"]["scale_y"])]
            trials = []
            for name in ("first_single_click", "subsequent_single_click", "canvas_drag"):
                # Distinct diagnostic trials; no failed action is automatically replayed.
                shot = snapshot()
                before = read_state(state_path)
                dom = before["dom"]
                if name != "canvas_drag":
                    rect = dom["regions"]["button"]
                    x, y = point(shot, rect["x"] + rect["width"] / 2, rect["y"] + rect["height"] / 2)
                    command = ["window", "click", "--target", shot["input_target"], "--x", x, "--y", y, "--count", "1"]
                else:
                    rect = dom["regions"]["canvas"]
                    obj = dom["object"]
                    x, y = point(shot, rect["x"] + obj["x"], rect["y"] + obj["y"])
                    to_x, to_y = point(shot, rect["x"] + obj["x"] + 80, rect["y"] + obj["y"] + 40)
                    command = ["window", "drag", "--target", shot["input_target"], "--x", x, "--y", y, "--to-x", to_x, "--to-y", to_y, "--duration-ms", "500"]
                result = run([str(cli)] + command)
                time.sleep(0.3)
                after = read_state(state_path)
                events = after["dom"]["events"][len(dom["events"]):]
                trials.append({"trial": name, "dispatch_status": result["status"], "posted_events": result["events_sent"],
                    "click_effects": after["dom"]["clicks"] - dom["clicks"],
                    "object_displacement": {axis: after["dom"]["object"][axis] - dom["object"][axis] for axis in ("x", "y")},
                    "release_effects": after["dom"]["releases"] - dom["releases"],
                    "dom_events": events, "target_active": after["active"],
                    "target_activations": after["activations"] - before["activations"],
                    "foreground_changes": after["foreground_changes"] - before["foreground_changes"],
                    "pointer_different_samples": after["pointer_different_samples"] - before["pointer_different_samples"],
                    "observation_samples": after["observations"] - before["observations"]})
            return {"macos": platform.mac_ver()[0], "architecture": platform.machine(), "inactive_space": inactive_space,
                    "observer_interval_ms": 10, "observation_limit": "timer sampling can miss transient changes; physical pointer movement is not attributed to the agent",
                    "trials": trials}
        finally:
            if receiver.poll() is None:
                os.killpg(receiver.pid, signal.SIGTERM)
            receiver.wait(timeout=5)
            for path in images:
                path.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--inactive-space", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    session = subprocess.run(["swift", "-e", 'import CoreGraphics; guard let s = CGSessionCopyCurrentDictionary() as? [String:Any] else { exit(1) }; print(s["CGSSessionScreenIsLocked"] as? Bool == true ? "locked" : "unlocked")'], capture_output=True, check=True, timeout=10)
    if session.stdout.decode().strip() != "unlocked":
        raise RuntimeError("desktop is locked; no fixture or input trial was started")
    report = probe(args.cli.resolve(), args.inactive_space)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    for trial in report["trials"]:
        print(trial["trial"], trial["click_effects"], trial["object_displacement"], len(trial["dom_events"]))


if __name__ == "__main__":
    main()
