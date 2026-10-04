#!/usr/bin/env python3
"""Run one fresh-agent native task with bounded, owned setup and independent checks."""
import argparse
import json
from pathlib import Path
import subprocess

from agent_session import AgentSession
from desktop_observer import DesktopObserver, compile_observer
from native_agent_task import (TASKS, compile_fixture, independent_evidence, launch_receiver,
                               merge_evidence, receiver_state, stop_receiver, task_setup)
from native_agent_task import unregister_fixture
from task_acceptance.evaluate import evaluate, load_json
from task_acceptance.setup import write_json


def run_task(cli, run, task_id, socket):
    manifest, goal, artifact, output = task_setup(run, task_id)
    process = None
    binary = None
    cleanup = {"task_id": task_id, "owned_process_stopped": True}
    try:
        binary = compile_fixture(output)
        observer_binary = compile_observer(output)
        process = launch_receiver(binary, output / "receiver.json", artifact)
        initial = receiver_state(output / "receiver.json", process)
        # The agent must perform opening too; setup has not read or prefilled its document.
        if initial.get("file") or initial.get("contents") or initial.get("read_requests") != 0:
            raise RuntimeError("receiver did not start with an unopened document")
        metadata = {"run_id": manifest["run_id"], "task_id": task_id, "attempted": True}
        write_json(output / "owned-scope.json", {**metadata, "pid": process.pid,
                   "window_id": initial["window_id"], "socket": str(socket), "goal": goal})
        with DesktopObserver(observer_binary, [process.pid], output / "session.trace.jsonl", duration_ms=900_000) as observer:
            session = AgentSession(cli, observer, process.pid, initial["window_id"], output / "session.json", metadata)
            session.serve_socket(socket, duration=800)
        # Retain activation failures in the actual trace/evaluator, rather than losing the attempt.
        final = receiver_state(output / "receiver.json", process, require_inactive=False)
        write_json(output / "receiver-final.json", final)
        evidence = independent_evidence(load_json(output / "session.json"), final, manifest, task_id, process.pid, artifact)
        path = merge_evidence(run, evidence)
        report = evaluate(Path(run), load_json(path))
        write_json(Path(run) / "native-agent-report.json", report)
        print(json.dumps({"task_id": task_id, "report": str(Path(run) / "native-agent-report.json")}), flush=True)
    except BaseException as error:
        write_json(output / "runner-error.json", {"error": str(error), "task_id": task_id})
        raise
    finally:
        stop_receiver(process)
        cleanup["owned_process_stopped"] = process is None or process.poll() is not None
        cleanup["owned_pid"] = process.pid if process else None
        if binary:
            try:
                cleanup["unique_bundle_unregistration_exit"] = unregister_fixture(binary)
            except (OSError, ValueError, subprocess.TimeoutExpired) as error:
                cleanup["bundle_cleanup_error"] = str(error)
        write_json(output / "cleanup.json", cleanup)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--task", choices=TASKS, required=True)
    parser.add_argument("--socket", type=Path, required=True)
    args = parser.parse_args()
    subprocess.run(["swift", "-e", 'import ApplicationServices; import CoreGraphics; guard let s=CGSessionCopyCurrentDictionary() as? [String:Any], s["CGSSessionScreenIsLocked"] as? Bool != true, AXIsProcessTrusted(), CGPreflightScreenCaptureAccess() else { exit(1) }'],
                   capture_output=True, check=True, timeout=10)
    run_task(args.cli.resolve(), args.run.resolve(), args.task, args.socket)


if __name__ == "__main__":
    main()
