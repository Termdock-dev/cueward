"""Owned two-window/dialog setup and independent evidence, without a scripted task."""
import json
from pathlib import Path

from desktop_observer import DesktopObserver, compile_observer
from native_agent_task import (compile_fixture, launch_receiver, merge_evidence, receiver_state,
                               session_evidence, task_setup)
from task_acceptance.evaluate import load_json
from task_acceptance.setup import write_json
from window_dialog_observer import WindowBindings, compile_binding_observer, owned_inventory
from window_dialog_session import DialogSession


def dialog_evidence(session, final, manifest, pid, artifact, windows, phases):
    result = session_evidence(session, manifest, "window_dialog", pid)
    if owned_inventory(final, pid, artifact) != windows:
        raise ValueError("final document native identities differ from setup")
    documents = final.get("documents")
    if not isinstance(documents, list) or len(documents) != 2:
        raise ValueError("both document IO observations required")
    target = next((d for d in documents if d.get("file") == str(artifact)), {})
    bystander = next((d for d in documents if d.get("file") == str(artifact.with_name("bystander.txt"))), {})
    identities = all(sum(all(d.get(k) == w[k] for k in ("pid", "window_id", "file", "ax_identifier"))
                         for d in documents) == 1 for w in windows)
    io = (identities and target.get("file") == str(artifact) and all(type(target.get(k)) is int and target[k] > 0
          for k in ("read_requests", "save_requests", "data_requests")) and target.get("error") == ""
          and target.get("document_edited") is False and final.get("dialog") is None
          and type(target.get("rejected_requests")) is int and target["rejected_requests"] == 0
          and type(bystander.get("rejected_requests")) is int and bystander["rejected_requests"] == 0
          and bystander.get("data_requests") == 0 and bystander.get("error") == ""
          and bystander.get("save_requests") == 0 and bystander.get("read_requests") == 1
          and bystander.get("contents") == bystander.get("initial_contents")
          and bystander.get("document_edited") is False)
    # Incomplete IO stays a failed attempt, not a passing observer synthesized from disk bytes.
    result["observer"] = {"source": "receiver_observer", "run_id": manifest["run_id"],
                          "task_id": "window_dialog", "pid": pid, "owned_windows": windows,
                          "window_observations": phases, "native_io_complete": io,
                          **{k: final.get(k) for k in ("active", "activations", "window_id", "observation_sequence",
                                                       "fresh_after_sequence")}}
    result["coverage"] = {"documents_preopened_by_setup": True, "native_file_dialog": any(
        p["phase"] == "dialog" for p in phases), "first_save_panel": False}
    if not io:
        result["failure_category"] = "partial_observation"
    return result


def run_dialog_task(cli, run, socket, *, write_report, record_failure, cleanup_receiver):
    """Share the entry runner's checkpoint/report/cleanup without loading it twice."""
    manifest, goal, artifact, output = task_setup(run, "window_dialog")
    process, binary = None, None
    try:
        binary = compile_fixture(output, "window_dialog")
        observer_binary = compile_observer(output)
        binding_binary = compile_binding_observer(output)
        process = launch_receiver(binary, output / "receiver.json", artifact,
                                  bystander=artifact.with_name("bystander.txt"))
        initial = receiver_state(output / "receiver.json", process)
        windows = owned_inventory(initial, process.pid, artifact)
        if initial.get("dialog") is not None or any(d.get("read_requests") != 1 for d in initial.get("documents", [])):
            raise RuntimeError("receiver did not start with two normally opened documents and no panel")
        bindings = WindowBindings(binding_binary, process, artifact, output / "receiver.json", output / "bindings", initial)
        bindings.read()  # Fail before serving rather than grant an unbound fresh Agent scope.
        metadata = {"run_id": manifest["run_id"], "task_id": "window_dialog", "attempted": True}
        write_json(output / "owned-scope.json", {**metadata, "pid": process.pid,
                   "window_id": initial["window_id"], "owned_windows": windows, "socket": str(socket), "goal": goal,
                   "allowed": "Owned app/window observations and freshly observed AX controls only; no menu/global/pointer actions."})
        with DesktopObserver(observer_binary, [process.pid], output / "session.trace.jsonl", duration_ms=900_000) as observer:
            session = DialogSession(cli, observer, process.pid, initial["window_id"], output / "session.json", metadata,
                                    bindings=bindings, artifact=artifact)
            session.serve_socket(socket, duration=800)
        final = receiver_state(output / "receiver.json", process, require_inactive=False,
                               fresh=True, expected_window_id=initial["window_id"])
        write_json(output / "receiver-final.json", final)
        evidence = dialog_evidence(load_json(output / "session.json"), final, manifest, process.pid,
                                   artifact, windows, session.phases)
        path = merge_evidence(run, evidence)
        print(json.dumps({"task_id": "window_dialog", "report": str(write_report(run, path))}), flush=True)
    except BaseException as error:
        record_failure(run, output, manifest, "window_dialog", process, error)
        if not (output / "session.json").exists():
            path = merge_evidence(run, {"run_id": manifest["run_id"], "task_id": "window_dialog", "attempted": False,
                                       "prerequisite": "fixture_unavailable", "blocked_reason": str(error)})
            write_report(run, path)
        raise
    finally:
        cleanup_receiver(process, binary, output, "window_dialog")
