#!/usr/bin/env python3
"""Measure real Cueward helpers and CLI calls using disposable AppKit windows."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
WINDOW = ROOT / "crates/adapter-macos/src/window"
SCREENSHOT = ROOT / "crates/adapter-macos/src/screenshot"
PREFIX = 'import Foundation\nlet perfStarted = ProcessInfo.processInfo.systemUptime\n'
REPORT = '''
let perfFinished = ProcessInfo.processInfo.systemUptime
let perfStats: [String: Double] = [
    "platform_and_validation_ms": (perfEncodeStarted - perfStarted) * 1000,
    "serialization_ms": (perfFinished - perfEncodeStarted) * 1000,
]
let perfData = try! JSONSerialization.data(withJSONObject: perfStats)
FileHandle.standardError.write(Data("CUEWARD_BENCHMARK ".utf8) + perfData + Data("\\n".utf8))
'''


def run(command, payload=None, timeout=45):
    started = time.perf_counter()
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(payload, timeout=timeout)
    except BaseException:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise
    elapsed = (time.perf_counter() - started) * 1000
    if process.returncode:
        # Only disposable helper errors are retained in the local terminal.
        raise RuntimeError(f"command exited {process.returncode}: {stderr.decode(errors='replace')}")
    return elapsed, stdout, stderr


def external(stdout):
    text = stdout.decode().strip()
    if text.startswith("<external "):
        text = text.split("\n", 1)[1].rsplit("\n</external>", 1)[0]
    return json.loads(text)


def instrument(source, catalog=False):
    if catalog:
        marker = "let data = try JSONSerialization.data(withJSONObject: payload, options: [])"
        if source.count(marker) != 1:
            raise RuntimeError("catalog source changed; update timing insertion")
        return PREFIX + source.replace(marker, "let perfEncodeStarted = ProcessInfo.processInfo.systemUptime\n" + marker) + REPORT
    marker = "func emit(_ result: [String: Any]) {"
    output = "    print(output)"
    if source.count(marker) != 1 or source.count(output) != 1:
        raise RuntimeError("AX helper source changed; update timing insertion")
    return PREFIX + source.replace(marker, marker + "\n    let perfEncodeStarted = ProcessInfo.processInfo.systemUptime").replace(output, REPORT + output)


def profile(command, request):
    elapsed, stdout, stderr = run(command, json.dumps(request).encode())
    reports = [line for line in stderr.decode().splitlines() if line.startswith("CUEWARD_BENCHMARK ")]
    if len(reports) != 1:
        raise RuntimeError("helper did not emit exactly one timing report")
    timing = json.loads(reports[0].split(" ", 1)[1])
    timing["round_trip_ms"] = elapsed
    timing["startup_and_transport_ms"] = elapsed - timing["platform_and_validation_ms"] - timing["serialization_ms"]
    return timing, json.loads(stdout)


def summary(samples):
    result = {}
    for key in samples[0]:
        values = sorted(sample[key] for sample in samples)
        result[key] = {
            "first": values[0] if len(values) == 1 else samples[0][key],
            "median": statistics.median(values),
            "p95": values[math.ceil(len(values) * 0.95) - 1],
            "min": min(values), "max": max(values),
        }
    return result


def read_state(path):
    return json.loads(path.read_text())


def key_counts(path):
    events = read_state(path)["events"]
    return sum(event["type"] == 10 for event in events), sum(event["type"] == 11 for event in events)


def check_delivery(path, before):
    deadline = time.monotonic() + 0.5
    while True:
        after = key_counts(path)
        if after == (before[0] + 1, before[1] + 1) or time.monotonic() > deadline:
            return {"key_downs_delta": after[0] - before[0], "key_ups_delta": after[1] - before[1]}
        time.sleep(0.01)


def measure(cli, directory, sample_count):
    fixture_source = directory / "receiver.swift"
    fixture_source.write_text((WINDOW / "input_fixture.swift").read_text().replace("+ 90)", "+ 600)"))
    fixture_binary = directory / "Receiver"
    run(["swiftc", str(fixture_source), "-o", str(fixture_binary)])
    state_path = directory / "state.json"
    receiver = subprocess.Popen([str(fixture_binary), str(state_path), "normal"], start_new_session=True)
    image_paths = set()
    def take_snapshot(window_id):
        _, stdout, _ = run([str(cli), "window", "snapshot", "--id", str(window_id)])
        snapshot = external(stdout)
        image_paths.add(Path(snapshot["screenshot"]["path"]))
        return snapshot
    try:
        deadline = time.monotonic() + 10
        while not state_path.exists():
            if time.monotonic() > deadline or receiver.poll() is not None:
                raise RuntimeError("fixture failed to start")
            time.sleep(0.05)
        time.sleep(0.2)
        state = read_state(state_path)
        if state["active"]:
            raise RuntimeError("fixture became foreground; background measurements aborted")
        window_id = state["windows"][1]
        snapshot = take_snapshot(window_id)
        window = snapshot["window"]
        bounds = window["bounds"]
        arguments = [str(receiver.pid), str(window_id), window["title"]] + [str(bounds[key]) for key in ("x", "y", "width", "height")]
        support = "\n".join((WINDOW / file).read_text() for file in ("ax_elements.swift", "ax_shared.swift", "ax_common.swift"))
        sources = {
            "catalog": instrument((SCREENSHOT / "window_catalog.swift").read_text(), catalog=True),
            "inspect": instrument(support + "\n" + (WINDOW / "ax_inspect.swift").read_text()),
            "wait": instrument(support + "\n" + (WINDOW / "wait_logic.swift").read_text() + "\n" + (WINDOW / "ax_wait.swift").read_text()),
            "key": instrument(support + "\n" + (WINDOW / "background_input.swift").read_text()),
        }
        workloads = {
            "catalog": (["all"], {}),
            "inspect": (arguments + ["200", "8", "0", "window"], {}),
            "wait": (arguments, {"options": {"condition": "element-exists", "selector": {"role": "AXTextArea", "name": None, "identifier": None}, "value": None, "timeout_ms": 1000, "interval_ms": 50}, "caller_pid": os.getpid()}),
            "key": (arguments, {"action": "key", "key_code": 124, "flags": 0, "caller_pid": os.getpid(), "lock_path": str(directory / "input.lock")}),
        }
        results = {}
        for name, source in sources.items():
            print(f"measuring helper {name}", flush=True)
            path = directory / f"{name}.swift"
            binary = directory / name
            path.write_text(source)
            compile_ms, _, _ = run(["swiftc", str(path), "-o", str(binary)])
            args, request = workloads[name]
            modes = {}
            for mode, command in (("interpreted", ["swift", str(path)]), ("compiled", [str(binary)])):
                samples = []
                for _ in range(sample_count):
                    if name == "key":
                        request["issued_at"] = time.time()
                        before_keys = key_counts(state_path)
                    timing, payload = profile(command + args, request)
                    if name == "wait" and payload["status"] != "matched":
                        raise RuntimeError("wait condition failed")
                    if name == "key" and (payload["events_sent"] != 2 or payload["status"] != "sent_unverified" or payload["foreground_changed"]):
                        raise RuntimeError("key delivery failed or foreground changed")
                    if name == "catalog" and not any(row["window_id"] == window_id for row in payload):
                        raise RuntimeError("fixture missing from catalog")
                    if name == "key":
                        timing.update(check_delivery(state_path, before_keys))
                    samples.append(timing)
                modes[mode] = {"samples": samples, "summary": summary(samples)}
            results[name] = {"instrumented_source_sha256": hashlib.sha256(source.encode()).hexdigest(),
                             "explicit_compilation_ms": compile_ms, "modes": modes}
        # Snapshot includes catalog checks, screencapture, file publication and token encoding.
        # Capture executable time is also measured separately; its internal API costs are opaque.
        cli_results = {}
        for name in ("catalog", "inspect", "snapshot", "wait", "key"):
            print(f"measuring CLI {name}", flush=True)
            samples = []
            for _ in range(sample_count):
                fresh = take_snapshot(window_id)
                token = fresh["input_target"]
                commands = {
                    "catalog": ["window", "list", "--all-spaces"],
                    "inspect": ["window", "inspect", "--id", str(window_id)],
                    "snapshot": ["window", "snapshot", "--id", str(window_id)],
                    "wait": ["window", "wait", "--target", token, "--condition", "element-exists", "--role", "AXTextArea"],
                    "key": ["window", "key", "--target", token, "--key", "right"],
                }
                before_keys = key_counts(state_path)
                elapsed, stdout, _ = run([str(cli)] + commands[name])
                payload = external(stdout)
                if name == "key" and (payload["events_sent"] != 2 or payload["foreground_changed"]):
                    raise RuntimeError("CLI input result failed")
                timing = {"round_trip_ms": elapsed}
                if name == "key":
                    timing.update(check_delivery(state_path, before_keys))
                samples.append(timing)
                Path(fresh["screenshot"]["path"]).unlink(missing_ok=True)
                if name == "snapshot":
                    image_paths.add(Path(payload["screenshot"]["path"]))
                    Path(payload["screenshot"]["path"]).unlink(missing_ok=True)
            cli_results[name] = {"samples": samples, "summary": summary(samples)}
        captures = []
        for _ in range(sample_count):
            elapsed, _, _ = run(["screencapture", "-x", "-t", "png", "-o", "-a", "-l", str(window_id), str(directory / "capture.png")])
            captures.append({"round_trip_ms": elapsed})
        time.sleep(0.1)
        final = read_state(state_path)
        expected_pairs = sample_count * 3  # Two helper modes plus CLI keys.
        downs = [event for event in final["events"] if event["type"] == 10]
        ups = [event for event in final["events"] if event["type"] == 11]
        verified = len(downs) == expected_pairs and len(ups) == expected_pairs and not final["active"] and final["texts"][1] == ""
        Path(snapshot["screenshot"]["path"]).unlink(missing_ok=True)
        return {"helpers": results, "cli": cli_results,
                "capture_executable": {"samples": captures, "summary": summary(captures)},
                "receiver_verification": {"passed": verified, "expected_key_pairs": expected_pairs,
                    "key_downs": len(downs), "key_ups": len(ups), "text_unchanged": final["texts"][1] == "",
                    "inactive_at_final_observation": not final["active"]}}
    finally:
        if receiver.poll() is None:
            os.killpg(receiver.pid, signal.SIGTERM)
        receiver.wait(timeout=5)
        for path in image_paths:
            path.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, default=ROOT / "target/release/cueward")
    parser.add_argument("--samples", type=int, default=10)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not 2 <= args.samples <= 50:
        parser.error("samples must be between 2 and 50")
    if platform.system() != "Darwin":
        parser.error("requires a logged-in macOS desktop")
    _, session, _ = run(["swift", "-e", 'import CoreGraphics; guard let s = CGSessionCopyCurrentDictionary() as? [String:Any] else { exit(1) }; print(s["CGSSessionScreenIsLocked"] as? Bool == true ? "locked" : "unlocked")'])
    if session.decode().strip() != "unlocked":
        parser.error("desktop is locked; no fixture or input workload was started")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    with tempfile.TemporaryDirectory(prefix="cueward-benchmark-") as temporary:
        results = measure(args.cli.resolve(), Path(temporary), args.samples)
    report = {"schema_version": 1, "macos": platform.mac_ver()[0], "architecture": platform.machine(),
              "base_revision": revision, "samples_per_workload": args.samples,
              "cli_sha256": hashlib.sha256(args.cli.read_bytes()).hexdigest(),
              "timing_scope": "subprocess and native helper execution; excludes model inference",
              "cache_scope": "first invocation and repetitions; OS and Swift caches are not reset",
              **results}
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"saved {args.output}", flush=True)
    if not report["receiver_verification"]["passed"]:
        raise RuntimeError(f"independent receiver result failed: {report['receiver_verification']}")


if __name__ == "__main__":
    main()
