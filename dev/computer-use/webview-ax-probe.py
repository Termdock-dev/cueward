#!/usr/bin/env python3
"""Observe lazy AX exposure and exactly-once AXPress on an ordinary WKWebView."""

import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import uuid
from types import SimpleNamespace

from desktop_observer import DesktopObserver, compile_observer, observation_checks
from observed_cli import ObservedCLI
from task_acceptance.setup import write_json
from webview_ax import BUTTON_NAME, click_effect, select_button, wait_click

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("webview_context", HERE / "webview-context-probe.py")
WEB = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WEB)


def inspect(commands, state):
    return commands.call("window", "inspect", "--id", state["window_id"], "--limit", "500", "--depth", "12")


def press_once(commands, state_path, report, name, identity):
    """One dispatch only; capture uncertainty without fallback or replay."""
    before = WEB.OWNED.read_state(state_path)
    action = {"name": name, "before": before}
    report["actions"].append(action)
    if (before.get("pid"), before.get("window_id")) != identity or before.get("active") is not False:
        raise RuntimeError("owned WebView identity/activity changed; no inspection or AX press")
    action["inspection"] = inspect(commands, before)
    button = select_button(action["inspection"], before)
    current = WEB.OWNED.read_state(state_path)
    if (current.get("pid"), current.get("window_id")) != identity or current.get("active") is not False:
        raise RuntimeError("owned WebView identity/activity changed; no AX press")
    try:
        action["result"] = commands.call("window", "press", "--target", button["target"])
    except RuntimeError as error:
        action["error"] = str(error)
    finally:
        action["receipt"] = commands.receipts[-1]
        action["after"], action["settling"] = wait_click(lambda: WEB.OWNED.read_state(state_path), before)
        action["effect"] = click_effect(before, action["after"])
    return action


def receiver_case(cli, fixture, observer_binary, directory, context):
    report = {"kind": "ordinary_webview_ax_diagnostic", "context": context, "actions": [], "status": "preparing"}
    state_path = directory / "state.json"
    images = set()
    process = subprocess.Popen([str(fixture), str(state_path), str(HERE / "webview-fixture.html"), "ordinary"],
                               start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        initial = WEB.BASE.wait_state(state_path)
        if initial.get("pid") != process.pid or initial.get("active") is not False:
            raise RuntimeError("owned WebView identity/activity unproven during setup; no input")
        identity = (process.pid, initial["window_id"])
        with DesktopObserver(observer_binary, [process.pid], directory / "trace.jsonl", duration_ms=90000) as observer:
            commands = ObservedCLI(cli, observer)
            if context == "inactive":
                report["target_space"] = WEB.OWNED.owned_move(commands, initial["window_id"], report)
            start = observer.mark("ax-case-start")
            try:
                report["cold_inspection"] = inspect(commands, initial)
                shot = commands.call("window", "snapshot", "--id", initial["window_id"])
                if shot["window"]["owner_pid"] != process.pid:
                    raise RuntimeError("owned WebView snapshot PID mismatch")
                images.add(Path(shot["screenshot"]["path"]))
                report["snapshot"] = shot
                # A supplied synthetic task goal names the button. No receiver flag,
                # accessibility setter, activation, injected script or input retry.
                report["wait"] = commands.call("window", "wait", "--target", shot["input_target"],
                                               "--condition", "element-exists", "--role", "AXButton",
                                               "--name", BUTTON_NAME, "--timeout-ms", "3000")
                if report["wait"]["status"] != "matched":
                    raise RuntimeError("page button did not become uniquely observable within budget")
                for name in ("first_ax_press", "subsequent_ax_press"):
                    action = press_once(commands, state_path, report, name, identity)
                    write_json(directory / "report.json", report)
                    if (action.get("error") or action["effect"]["status"] != "passed"
                            or action.get("result", {}).get("foreground_changed") is not False):
                        report["remaining_actions_not_dispatched"] = [] if name == "subsequent_ax_press" else ["subsequent_ax_press"]
                        break
                report["status"] = "recorded"
            finally:
                end = observer.mark("ax-case-end")
                report["execution"] = observer.execution(start, end)
                report["desktop"] = observation_checks(report["execution"], [process.pid])
                report["receipts"] = commands.receipts
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        report.update(status="diagnostic_error", error=str(error))
    finally:
        WEB.OWNED.cleanup([process], {"receiver": state_path}, SimpleNamespace(images=images), report)
        write_json(directory / "report.json", report)
    return report


def probe(cli, parent, context):
    root = parent / ("webview-ax-" + uuid.uuid4().hex)
    root.mkdir(mode=0o700)
    fixture = root / "OrdinaryWebViewFixture"
    subprocess.run(["swiftc", "-warnings-as-errors", "-module-cache-path", str(root / "module-cache"),
                    str(HERE / "webview-event-fixture.swift"), "-o", str(fixture)],
                   check=True, capture_output=True, timeout=90)
    observer_binary = compile_observer(root)
    return root, receiver_case(cli, fixture, observer_binary, root, context)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--parent", type=Path, required=True)
    parser.add_argument("--context", choices=("visible", "inactive"), default="inactive")
    args = parser.parse_args()
    subprocess.run(["swift", "-e", 'import ApplicationServices; import CoreGraphics; guard let s = CGSessionCopyCurrentDictionary() as? [String:Any], s["CGSSessionScreenIsLocked"] as? Bool != true, AXIsProcessTrusted(), CGPreflightScreenCaptureAccess(), CGPreflightPostEventAccess() else { exit(1) }'],
                   check=True, capture_output=True, timeout=10)
    root, report = probe(args.cli.resolve(), args.parent.resolve(), args.context)
    print(json.dumps({"run": str(root), "context": args.context, "status": report["status"],
                      "error": report.get("error"), "wait": report.get("wait"), "desktop": report.get("desktop"),
                      "actions": [{"name": a["name"], "effect": a.get("effect"), "error": a.get("error")}
                                  for a in report["actions"]], "cleanup": report["cleanup"]}), flush=True)


if __name__ == "__main__":
    main()
