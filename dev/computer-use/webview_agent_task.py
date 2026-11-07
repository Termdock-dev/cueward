"""Owned WebView pointer tasks: setup and independent evidence, without a scripted task."""
import json
from pathlib import Path
import subprocess
import time

from agent_session import AgentSession, ScopeError
from desktop_observer import DesktopObserver, compile_observer
from native_agent_task import merge_evidence, receiver_state, session_evidence
from task_acceptance.evaluate import fresh_canvas, load_json
from task_acceptance.setup import write_json

HERE = Path(__file__).resolve().parent
TASKS = {"webview_first_click": "click", "canvas_drag": "drag", "canvas_interrupted": "drag"}
# Page effects can land after the CLI returns: webview-context.md measured up to 906 ms.
SETTLE_SECONDS = 3


def task_setup(run, task_id):
    """Validate a prepared run and reserve private metadata; these tasks have no artifact."""
    run = Path(run).resolve()
    manifest = load_json(run / "manifest.json")
    goals = [g for g in manifest["tasks"] if g["task_id"] == task_id]
    if manifest.get("schema") != 1 or task_id not in TASKS or len(goals) != 1:
        raise ValueError("expected one supported WebView task in a prepared run")
    parent = run / "operator"
    if parent.is_symlink() or parent.exists() and not parent.is_dir():
        raise ValueError("task metadata parent must be an unlinked directory")
    # Reserve once: reruns require a fresh prepared run, not a repaired old result.
    output = parent / task_id
    output.mkdir(parents=True, exist_ok=False)
    return manifest, goals[0], output


def compile_webview_fixture(output):
    """Build a bare owned binary. With no bundle, LaunchServices has nothing to unregister."""
    output = Path(output)
    binary = output / "WebViewFixture"
    compiled = subprocess.run(["swiftc", "-warnings-as-errors", "-module-cache-path", str(output / "module-cache"),
                               str(HERE / "webview-fixture.swift"), "-o", str(binary)], capture_output=True, timeout=90)
    (output / "fixture-compiler.stdout.log").write_bytes(compiled.stdout)
    (output / "fixture-compiler.stderr.log").write_bytes(compiled.stderr)
    compiled.check_returncode()
    return binary


def launch_webview(binary, state, task_id, lifetime=900):
    """Return the owned process handle; only the interrupted task opts into the title change."""
    mode = ["interrupt"] if task_id == "canvas_interrupted" else []
    with Path(state).with_suffix(".stdout.log").open("x") as stdout, \
            Path(state).with_suffix(".stderr.log").open("x") as stderr:
        return subprocess.Popen([str(binary), str(state), str(HERE / "webview-fixture.html"), str(lifetime), *mode],
                                start_new_session=True, stdout=stdout, stderr=stderr)


def loaded_state(path, process, timeout=10):
    """Return the first inactive owned state with a page; the file appears before the page loads."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        state = receiver_state(path, process)
        if isinstance(state.get("dom"), dict) and state["dom"].get("regions"):
            return state
        time.sleep(0.03)
    raise RuntimeError("WebView page did not load")


def fresh_page(task_id, dom):
    """Pointer hover adds page events, which only the canvas contract forbids."""
    return dom.get("clicks") == 0 if task_id == "webview_first_click" else fresh_canvas(dom)


def final_state(path, process, window_id):
    """Wait for late page effects, then require page state newer than the closed observer."""
    baseline = load_json(path).get("dom_updates")
    time.sleep(SETTLE_SECONDS)
    state = receiver_state(path, process, require_inactive=False, fresh=True, expected_window_id=window_id)
    if type(baseline) is not int or type(state.get("dom_updates")) is not int or state["dom_updates"] <= baseline:
        raise RuntimeError("page stopped reporting; its final state would be stale")
    return state


class PointerSession(AgentSession):
    """Allow one pointer route. An AX action would change the page without a counted dispatch."""
    def __init__(self, *args, action, **kwargs):
        super().__init__(*args, **kwargs)
        self.action = action

    def invoke(self, arguments):
        other = "drag" if self.action == "click" else "click"
        if isinstance(arguments, list) and arguments[:2] in (["app", "press"], ["app", "set-value"], ["window", other]):
            raise ScopeError(f"only one window {self.action} may reach the page in this task")
        return super().invoke(arguments)

    def serve_socket(self, path, duration=800):
        try:
            super().serve_socket(path, duration)
        finally:
            self.remove_snapshots()

    def remove_snapshots(self):
        """Delete only images the CLI reported for the owned window; they sit in the user's cache."""
        for record in self.records:
            result = record.get("result")
            if (record["arguments"][:2] == ["window", "snapshot"] and isinstance(result, dict)
                    and result.get("window", {}).get("owner_pid") == self.pid):
                Path(result["screenshot"]["path"]).unlink(missing_ok=True)


def pointer_evidence(session, initial, final, manifest, task_id, pid):
    """Join actual session records to page states read outside the Agent's reach."""
    result = session_evidence(session, manifest, task_id, pid)
    dispatches = session.get("dispatches")
    if not isinstance(dispatches, dict) or not all(type(n) is int and n >= 0 for n in dispatches.values()):
        raise ValueError("session has no dispatch record")
    result["observer"] = {"source": "receiver_observer", "run_id": manifest["run_id"], "task_id": task_id,
                          "pid": pid, "initial": initial["dom"], "final": final["dom"],
                          "dispatch_count": sum(dispatches.values()),
                          **{k: final.get(k) for k in ("active", "activations", "window_id", "observation_sequence",
                                                       "fresh_after_sequence")}}
    if task_id == "canvas_interrupted":
        result["observer"]["controlled_interruption"] = (initial.get("interruption_triggered") is False
                                                         and final.get("interruption_triggered") is True)
    return result


def run_webview_task(cli, run, task_id, socket, *, write_report, record_failure, cleanup_receiver):
    """Share the entry runner's checkpoint, report and cleanup, as the dialog task does."""
    manifest, goal, output = task_setup(run, task_id)
    state = output / "receiver.json"
    process = None
    try:
        binary = compile_webview_fixture(output)
        observer_binary = compile_observer(output)
        process = launch_webview(binary, state, task_id)
        initial = loaded_state(state, process)
        if not fresh_page(task_id, initial["dom"]):
            raise RuntimeError("receiver did not start with a fresh page")
        metadata = {"run_id": manifest["run_id"], "task_id": task_id, "attempted": True}
        write_json(output / "owned-scope.json", {**metadata, "pid": process.pid, "window_id": initial["window_id"],
                   "socket": str(socket), "goal": {k: goal[k] for k in ("task_id", "family", "goal")},
                   "allowed": f"Owned window snapshot/inspect, app inspect, and one window {TASKS[task_id]} using a current snapshot target. No AX actions and no replay."})
        with DesktopObserver(observer_binary, [process.pid], output / "session.trace.jsonl", duration_ms=900_000) as observer:
            session = PointerSession(cli, observer, process.pid, initial["window_id"], output / "session.json",
                                     metadata, action=TASKS[task_id])
            session.serve_socket(socket, duration=800)
        final = final_state(state, process, initial["window_id"])
        write_json(output / "receiver-final.json", final)
        evidence = pointer_evidence(load_json(output / "session.json"), initial, final, manifest, task_id, process.pid)
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
        cleanup_receiver(process, None, output, task_id)
