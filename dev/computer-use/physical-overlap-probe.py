#!/usr/bin/env python3
"""One bounded human/background native-input round for #31; not fresh-agent acceptance."""
import argparse
import json
from pathlib import Path
import subprocess
import time
import uuid

from desktop_observer import DesktopObserver, compile_observer, observation_checks
from observed_cli import ObservedCLI
from physical_overlap import background_check, evaluate, interruption_check
from task_acceptance.setup import write_json

HERE = Path(__file__).resolve().parent


def read_state(path):
    return json.loads(path.read_text())


def wait_state(path, predicate=lambda value: True, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            state = read_state(path)
            if predicate(state):
                return state
        time.sleep(0.02)
    raise RuntimeError("owned receiver did not reach the required state before deadline")


def owned_move(commands, window_id, report):
    catalog = commands.call("space", "list")
    choices = [s["id"] for d in catalog["displays"] for s in d["spaces"] if s["type"] == 0 and not s["is_visible"]]
    if not choices:
        raise RuntimeError("no existing inactive user Space; no Space created")
    membership = commands.call("space", "window", "--id", window_id)
    token = membership.get("move_target")
    if not token:
        raise RuntimeError("no fresh owned move target")
    moved = commands.call("space", "move-window", "--target", token, "--space", choices[0])
    report.setdefault("setup_moves", []).append(moved)
    # Preparation may overlap ordinary user App switching. Retain it, do not
    # import preparation transitions into the later fixed-foreground round.
    verified = commands.call("space", "window", "--id", window_id)
    if (moved.get("status") != "confirmed" or moved.get("visible_spaces_changed")
            or moved.get("window_changed") or verified.get("space_ids") != [choices[0]]):
        raise RuntimeError("owned background membership/visible Spaces not confirmed")
    return str(choices[0])


class Round:
    def __init__(self, cli, observer, report, paths):
        self.commands = ObservedCLI(cli, observer)
        self.observer, self.report, self.paths = observer, report, paths
        self.images = set()
        self.input_deadline = None

    def snapshot(self, mode):
        state = read_state(self.paths[mode])
        shot = self.commands.call("window", "snapshot", "--id", state["window_id"])
        if shot["window"]["owner_pid"] != state["pid"]:
            raise RuntimeError("owned snapshot PID mismatch")
        self.images.add(Path(shot["screenshot"]["path"]))
        return shot

    def action(self, name, mode, build):
        shot = self.snapshot(mode)
        state = read_state(self.paths[mode])
        record = {"name": name, "snapshot": shot, "before": state}
        self.report["actions"].append(record)
        try:
            if self.input_deadline is not None and time.monotonic() >= self.input_deadline:
                record["pre_dispatch_refusal"] = True
                raise RuntimeError("round deadline after snapshot; no input dispatched")
            record["result"] = self.commands.call("window", *build(shot, state))
        except RuntimeError as error:
            record["error"] = str(error)
        finally:
            record["receipt"] = self.commands.receipts[-1]
            time.sleep(0.1)
            record["after"] = read_state(self.paths[mode])

    def point(self, shot, state, region, x=None, y=None):
        rectangle = state["regions"][region]
        return (str((rectangle["x"] + (rectangle["width"] / 2 if x is None else x)) * shot["image"]["scale_x"]),
                str((rectangle["y"] + (rectangle["height"] / 2 if y is None else y)) * shot["image"]["scale_y"]))

    def pointer(self, kind, shot, state):
        region = "button" if kind == "click" or kind == "foreground_refusal" else "canvas" if kind in ("drag", "interrupted_drag") else "scroll"
        obj = state["object"]
        x, y = self.point(shot, state, region, obj["x"] if region == "canvas" else None, obj["y"] if region == "canvas" else None)
        common = ["--target", shot["input_target"], "--x", x, "--y", y]
        if region == "button":
            return ["click", *common, "--count", "1"]
        if region == "scroll":
            return ["scroll", *common, "--delta-y", "-160"]
        to_x, to_y = self.point(shot, state, region, obj["x"] + 80, obj["y"] + 40)
        return ["drag", *common, "--to-x", to_x, "--to-y", to_y, "--duration-ms", "1000"]

    def main_actions(self):
        started = time.monotonic()
        self.input_deadline = started + 28
        for name in ("text", "shortcut", "replacement", "click", "drag", "scroll"):
            if time.monotonic() - started > 28:
                raise RuntimeError("round deadline; remaining actions not dispatched")
            if name in ("text", "replacement"):
                value = self.report["first_text" if name == "text" else "last_text"]
                build = lambda shot, state, value=value: ["type-text", "--target", shot["input_target"], "--text", value]
            elif name == "shortcut":
                build = lambda shot, state: ["key", "--target", shot["input_target"], "--key", "a", "--modifiers", "command"]
            else:
                build = lambda shot, state, name=name: self.pointer(name, shot, state)
            self.action(name, "background", build)
            time.sleep(1)
        self.action("interrupted_drag", "interrupt", lambda shot, state: self.pointer("interrupted_drag", shot, state))

    def finish_recording(self, start):
        end = self.observer.mark("physical-round-end")
        self.report["execution"] = self.observer.execution(start, end)
        self.report["marker_times"] = {r["id"]: r["ms"] for r in self.observer.records if r.get("type") == "mark"}
        self.report["receipts"] = self.commands.receipts
        for mode in self.paths:
            self.report[mode + "_final"] = read_state(self.paths[mode])

    def execute(self, idle=False):
        if not idle:
            human = wait_state(self.paths["human"], lambda s: s["phase"] == "running" and s["active"], timeout=180)
            self.report["human_pid"] = human["pid"]
            self.report["human_started_at"] = human["started_at"]
        self.report["background_initial"] = read_state(self.paths["background"])
        start = self.observer.mark("physical-round-start")
        started = time.monotonic()
        try:
            self.main_actions()
            if not idle:
                self.action("foreground_refusal", "human", lambda shot, state: self.pointer("foreground_refusal", shot, state))
                while time.monotonic() - started < 30:
                    time.sleep(0.05)
                wait_state(self.paths["human"], lambda s: s["phase"] == "acknowledged", timeout=60)
        finally:
            self.finish_recording(start)


def cleanup(processes, paths, runner, report):
    """A corrupt final state must not prevent stopping the owned subprocesses."""
    errors = []
    for mode, path in paths.items():
        try:
            if path.exists() and mode + "_final" not in report:
                report[mode + "_final"] = read_state(path)
        except (OSError, ValueError) as error:
            errors.append(str(error))
    for process in processes:
        try:
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill(); process.wait(timeout=5)
        except (OSError, subprocess.SubprocessError) as error:
            errors.append(str(error))
    if runner:
        for path in runner.images:
            try:
                path.unlink(missing_ok=True)
            except OSError as error:
                errors.append(str(error))
    report["cleanup"] = {"owned_processes_stopped": all(p.poll() is not None for p in processes), "errors": errors}


def probe(cli, directory, idle=False):
    report = {"kind": "idle_routing_diagnostic" if idle else "physical_diagnostic", "status": "preparing", "actions": [],
              "first_text": "BG31_START_" + uuid.uuid4().hex[:12], "last_text": "BG31_END_" + uuid.uuid4().hex[:12],
              "limitations": "Only owned synthetic receivers. Pointer correlation is sampled, not causal authentication. Foreground refusal is pre-dispatch, not a mid-flight activation case. No production guard bypass or replay."}
    output = directory / "report.json"
    write_json(output, report)
    processes, paths = [], {}
    runner = None
    try:
        fixture = directory / "OverlapFixture"
        subprocess.run(["swiftc", "-module-cache-path", str(directory / "module-cache"), str(HERE / "overlap-fixture.swift"), "-o", str(fixture)],
                       capture_output=True, check=True, timeout=90)
        binary = compile_observer(directory)
        for mode in ("background", "interrupt"):
            paths[mode] = directory / (mode + ".json")
            with (directory / (mode + ".log")).open("x") as log:
                processes.append(subprocess.Popen([str(fixture), str(paths[mode]), mode], stdout=log, stderr=log, start_new_session=True))
            wait_state(paths[mode])
        report["receiver_pids"] = [p.pid for p in processes]
        with DesktopObserver(binary, report["receiver_pids"], directory / "trace.jsonl", duration_ms=300_000) as observer:
            setup = ObservedCLI(cli, observer)
            report["setup_receipts"] = setup.receipts
            for mode in paths:
                state = read_state(paths[mode])
                space = owned_move(setup, state["window_id"], report)
                if "target_space" in report and space != report["target_space"]:
                    raise RuntimeError("different owned inactive Space destinations")
                report["target_space"] = space
            if not idle:
                paths["human"] = directory / "human.json"
                with (directory / "human.log").open("x") as log:
                    processes.append(subprocess.Popen([str(fixture), str(paths["human"]), "human"], stdout=log, stderr=log, start_new_session=True))
                wait_state(paths["human"])
                report["status"] = "waiting_for_human"
                write_json(output, report)
                print(json.dumps({"ready": True, "human_window_title": "Cueward #31 human input", "run": str(directory)}), flush=True)
            runner = Round(cli, observer, report, paths)
            runner.execute(idle=idle)
            if idle:
                report["assessment"] = {
                    "kind": "idle_diagnostic_only_not_physical_acceptance",
                    "background": background_check(report["background_initial"], report["background_final"], report["first_text"], report["last_text"]),
                    "controlled_interruption": interruption_check(report["actions"]),
                    "desktop": observation_checks(report["execution"], report["receiver_pids"]),
                }
            else:
                report["assessment"] = evaluate(report)
            report["status"] = "recorded"
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        report.update(status="diagnostic_error", error=str(error))
    finally:
        cleanup(processes, paths, runner, report)
        write_json(output, report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--parent", type=Path, required=True)
    parser.add_argument("--idle-diagnostic", action="store_true", help="No human receiver; routing/effect diagnosis only, not #31 acceptance")
    args = parser.parse_args()
    # Never begin a human task with locked session or missing existing permissions.
    subprocess.run(["swift", "-e", 'import ApplicationServices; import CoreGraphics; guard let s = CGSessionCopyCurrentDictionary() as? [String:Any], s["CGSSessionScreenIsLocked"] as? Bool != true, AXIsProcessTrusted(), CGPreflightScreenCaptureAccess(), CGPreflightPostEventAccess() else { exit(1) }'],
                   check=True, capture_output=True, timeout=10)
    directory = args.parent.resolve() / ("physical-overlap-" + uuid.uuid4().hex)
    directory.mkdir(mode=0o700)
    report = probe(args.cli.resolve(), directory, idle=args.idle_diagnostic)
    print(json.dumps({"run": str(directory), "status": report["status"], "assessment": report.get("assessment"), "error": report.get("error")}), flush=True)


if __name__ == "__main__":
    main()
