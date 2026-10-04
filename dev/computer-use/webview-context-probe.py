#!/usr/bin/env python3
"""Separate App/native first-mouse/DOM evidence; controls never count as ordinary support."""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import time
import uuid
from types import SimpleNamespace

from desktop_observer import DesktopObserver, compile_observer, observation_checks
from observed_cli import ObservedCLI
from task_acceptance.setup import write_json

HERE = Path(__file__).resolve().parent


def module(name, file):
    spec = importlib.util.spec_from_file_location(name, HERE / file)
    value = importlib.util.module_from_spec(spec); spec.loader.exec_module(value)
    return value


BASE = module("webview_baseline", "webview-probe.py")
OWNED = module("owned_native_probe", "physical-overlap-probe.py")


def effect_check(before, after, name):
    a, b = before["dom"], after["dom"]
    events = b["events"][len(a["events"]):]
    delta = {k: b["object"][k] - a["object"][k] for k in ("x", "y")}
    if name == "canvas_drag":
        held = [e for e in events if e["type"] == "mousemove"]
        downs = [e for e in events if e["type"] == "mousedown"]
        ups = [e for e in events if e["type"] == "mouseup"]
        passed = (abs(delta["x"] - 80) < 0.1 and abs(delta["y"] - 40) < 0.1
                  and b["releases"] - a["releases"] == 1 and b["dragging"] is False
                  and bool(held) and all(e["buttons"] & 1 for e in held)
                  and len(downs) == len(ups) == 1 and downs[0]["buttons"] & 1 and ups[0]["buttons"] == 0)
    else:
        passed = b["clicks"] - a["clicks"] == 1 and sum(e["type"] == "click" for e in events) == 1
    return {"status": "passed" if passed else "failed", "click_increment": b["clicks"] - a["clicks"],
            "object_delta": delta, "release_increment": b["releases"] - a["releases"],
            "dom_event_count": len(events), "dragging": b["dragging"],
            "dom_buttons_by_type": {t: sorted({e["buttons"] for e in events if e["type"] == t})
                                    for t in ("mousedown", "mousemove", "mouseup", "click")}}


def wait_terminal(state_path, before, name, timeout=3):
    """Wait for a fresh DOM terminal observation, never repost input."""
    started = time.monotonic()
    while True:
        after = OWNED.read_state(state_path)
        events = after["dom"]["events"][len(before["dom"]["events"]):]
        kinds = [e["type"] for e in events]
        complete = "mouseup" in kinds and (name == "canvas_drag" or "click" in kinds)
        elapsed = time.monotonic() - started
        if complete or elapsed >= timeout:
            return after, {"status": "terminal_dom_observed" if complete else "no_terminal_dom_within_budget",
                           "wait_ms": elapsed * 1000, "budget_ms": timeout * 1000}
        time.sleep(0.05)


def record_action(commands, state_path, name, images):
    before = OWNED.read_state(state_path)
    shot = commands.call("window", "snapshot", "--id", before["window_id"])
    if shot["window"]["owner_pid"] != before["pid"]:
        raise RuntimeError("owned WebView snapshot PID mismatch")
    images.add(Path(shot["screenshot"]["path"]))
    before = OWNED.read_state(state_path)
    action = {"name": name, "before": before, "snapshot": shot}
    try:
        if before["active"]:
            raise RuntimeError("owned WebView became active; no background dispatch")
        action["result"] = commands.call(*BASE.trial_command(shot, before, name))
    except RuntimeError as error:
        action["error"] = str(error)
    finally:
        action["receipt"] = commands.receipts[-1]
        action["after"], action["settling"] = wait_terminal(state_path, before, name)
        action["effect"] = effect_check(before, action["after"], name)
    return action


def receiver_case(cli, fixture, observer_binary, directory, variant, context, names):
    report = {"variant": variant, "context": context, "actions": [], "status": "preparing",
              "ordinary_support_candidate": variant == "ordinary"}
    state = directory / "receiver.json"; images = set()
    process = subprocess.Popen([str(fixture), str(state), str(HERE / "webview-fixture.html"), variant], start_new_session=True,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        before = BASE.wait_state(state)
        if before["active"]:
            raise RuntimeError("receiver active during setup; no input")
        with DesktopObserver(observer_binary, [process.pid], directory / "trace.jsonl", duration_ms=90000) as observer:
            commands = ObservedCLI(cli, observer)
            if context == "inactive":
                report["target_space"] = OWNED.owned_move(commands, before["window_id"], report)
            report["setup_receipt_count"] = len(commands.receipts)
            start = observer.mark("case-start")
            try:
                for name in names:
                    action = record_action(commands, state, name, images); report["actions"].append(action)
                    write_json(directory / "report.json", report)
                    if action.get("error") or action.get("result", {}).get("status") == "partially_sent":
                        report["remaining_actions_not_dispatched"] = list(names[len(report["actions"]):])
                        break
                    time.sleep(0.5)
            finally:
                end = observer.mark("case-end")
                report["execution"] = observer.execution(start, end)
                report["receipts"] = commands.receipts
                report["desktop"] = observation_checks(report["execution"], [process.pid])
            report["status"] = "recorded"
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        report.update(status="diagnostic_error", error=str(error))
    finally:
        OWNED.cleanup([process], {"receiver": state}, SimpleNamespace(images=images), report)
        write_json(directory / "report.json", report)
    return report


def probe(cli, parent, variant, context):
    root = parent / ("webview-context-" + uuid.uuid4().hex); root.mkdir(mode=0o700)
    fixture = root / "WebViewEventFixture"
    subprocess.run(["swiftc", "-warnings-as-errors", "-module-cache-path", str(root / "module-cache"),
                    str(HERE / "webview-event-fixture.swift"), "-o", str(fixture)], check=True, capture_output=True, timeout=90)
    observer = compile_observer(root)
    results = []
    for names in (("first_single_click", "subsequent_single_click"), ("canvas_drag",)):
        folder = root / names[0]; folder.mkdir()
        result = receiver_case(cli, fixture, observer, folder, variant, context, names)
        results.append(result)
    write_json(root / "report.json", {"kind": "webview_context_diagnostic", "cases": results})
    return root, results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--parent", type=Path, required=True)
    parser.add_argument("--variant", choices=("ordinary", "observed", "control"), default="ordinary")
    parser.add_argument("--context", choices=("inactive", "visible"), default="inactive")
    args = parser.parse_args()
    subprocess.run(["swift", "-e", 'import ApplicationServices; import CoreGraphics; guard let s = CGSessionCopyCurrentDictionary() as? [String:Any], s["CGSSessionScreenIsLocked"] as? Bool != true, AXIsProcessTrusted(), CGPreflightScreenCaptureAccess(), CGPreflightPostEventAccess() else { exit(1) }'],
                   check=True, capture_output=True, timeout=10)
    root, results = probe(args.cli.resolve(), args.parent.resolve(), args.variant, args.context)
    print(json.dumps({"run": str(root), "variant": args.variant, "context": args.context,
                      "cases": [{"status": r["status"], "error": r.get("error"), "desktop": r.get("desktop"),
                                 "actions": [{"name": a["name"], "effect": a["effect"]} for a in r["actions"]]} for r in results]}), flush=True)


if __name__ == "__main__":
    main()
