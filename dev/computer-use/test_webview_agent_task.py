"""Pointer task setup, scope, evidence and ordering; these are not desktop task passes."""
import base64
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

from agent_session import ScopeError
from task_acceptance.evaluate import evaluate
from task_acceptance.setup import prepare, write_json
import webview_agent_task as task

spec = importlib.util.spec_from_file_location("webview_runner", Path(__file__).with_name("native-agent-task.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)

PID, WINDOW = 200, 20
PAIR = [{"type": "mousedown", "trusted": True, "buttons": 0},
        {"type": "mousemove", "trusted": True, "buttons": 0},
        {"type": "mouseup", "trusted": True, "buttons": 0}]


def page(**changes):
    return {"clicks": 0, "releases": 0, "events": [], "dragging": False, "object": {"x": 100, "y": 80},
            "regions": {"button": {"x": 20}, "canvas": {"x": 20}}} | changes


def receiver(dom=None, **changes):
    return {"pid": PID, "window_id": WINDOW, "active": False, "activations": 0, "observation_sequence": 1,
            "dom_updates": 1, "interruption_triggered": False, "dom": page() if dom is None else dom} | changes


def token(value):
    return base64.urlsafe_b64encode(json.dumps(value).encode()).decode().rstrip("=")


class EvidenceCase(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.root = prepare(folder.name)
        self.manifest = json.loads((self.root / "manifest.json").read_text())

    def session(self, task_id, **changes):
        samples = [{"ms": ms, "frontmost_pid": 100, "pointer": [12, 34], "visible_spaces": {"display-1": "space-1"},
                    "target_active": {str(PID): False}} for ms in (0, 50, 100)]
        action = task.TASKS[task_id]
        receipt = {"command": f"/owned/cueward window {action} --target token", "status": "sent_unverified", "elapsed_ms": 10}
        if task_id == "canvas_interrupted":
            receipt.update(status="partially_sent", events_sent=3, interruption="window changed; take a new snapshot")
        return {"run_id": self.manifest["run_id"], "task_id": task_id, "attempted": True, "receiver_pids": [PID],
                "receipts": [receipt], "execution": {"start_ms": 0, "end_ms": 100, "samples": samples},
                "dispatches": {"click": 0, "drag": 0} | {action: 1}, "records": [{"result": "private raw result"}]} | changes

    def evaluated(self, task_id, session, initial, final):
        record = task.pointer_evidence(session, initial, final, self.manifest, task_id, PID)
        evidence = {"schema": 1, "run_id": self.manifest["run_id"], "kind": "agent", "tasks": {task_id: record}}
        result = next(t for t in evaluate(self.root, evidence)["tasks"] if t["task_id"] == task_id)
        return result, {c["name"]: c["status"] for c in result["checks"]}, record


class PointerEvidenceTests(EvidenceCase):
    def test_one_click_with_one_effect_passes_without_retaining_raw_records(self):
        session = self.session("webview_first_click")
        result, _, record = self.evaluated("webview_first_click", session, receiver(), receiver(page(clicks=1)))
        self.assertEqual(result["status"], "passed", result)
        self.assertEqual(record["observer"]["source"], "receiver_observer")
        self.assertEqual(record["observer"]["dispatch_count"], 1)
        self.assertNotIn("records", record)

    def test_dispatch_count_is_every_pointer_dispatch_the_session_recorded(self):
        for dispatches, expected in (({"click": 1, "drag": 1}, 2), ({"click": 2, "drag": 0}, 2), ({"click": 0, "drag": 0}, 0)):
            with self.subTest(dispatches=dispatches):
                session = self.session("webview_first_click", dispatches=dispatches)
                result, checks, record = self.evaluated("webview_first_click", session, receiver(), receiver(page(clicks=1)))
                self.assertEqual(record["observer"]["dispatch_count"], expected)
                self.assertEqual(checks["no_replay"], "failed")
                self.assertEqual(result["status"], "failed")

    def test_missing_dispatch_record_is_never_invented(self):
        for dispatches in (None, {"click": "1"}, {"click": True}, {"click": -1}):
            with self.subTest(dispatches=dispatches), self.assertRaisesRegex(ValueError, "dispatch record"):
                task.pointer_evidence(self.session("webview_first_click", dispatches=dispatches), receiver(),
                                      receiver(page(clicks=1)), self.manifest, "webview_first_click", PID)

    def test_session_from_another_receiver_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "identity"):
            task.pointer_evidence(self.session("canvas_drag", receiver_pids=[201]), receiver(), receiver(),
                                  self.manifest, "canvas_drag", PID)

    def test_canvas_drag_that_moves_nothing_stays_failed_after_a_clean_release(self):
        final = receiver(page(events=PAIR, releases=1))
        result, checks, _ = self.evaluated("canvas_drag", self.session("canvas_drag"), receiver(), final)
        self.assertEqual(result["status"], "failed")
        self.assertEqual(checks["displacement"], "failed")
        self.assertEqual(checks["release"], "passed")

    def test_canvas_drag_passes_only_on_the_requested_displacement(self):
        moved = receiver(page(events=PAIR, releases=1, object={"x": 180, "y": 120}))
        self.assertEqual(self.evaluated("canvas_drag", self.session("canvas_drag"), receiver(), moved)[0]["status"], "passed")
        short = receiver(page(events=PAIR, releases=1, object={"x": 179, "y": 120}))
        self.assertEqual(self.evaluated("canvas_drag", self.session("canvas_drag"), receiver(), short)[0]["status"], "failed")

    def test_interrupted_drag_passes_with_receiver_record_and_stopped_receipt(self):
        final = receiver(page(events=PAIR, releases=1), interruption_triggered=True)
        result, _, record = self.evaluated("canvas_interrupted", self.session("canvas_interrupted"), receiver(), final)
        self.assertEqual(result["status"], "passed", result)
        self.assertIs(record["observer"]["controlled_interruption"], True)

    def test_interruption_comes_from_the_receiver_not_from_the_receipt(self):
        session = self.session("canvas_interrupted")
        for initial, final in ((receiver(), receiver(page(events=PAIR, releases=1))),
                               (receiver(interruption_triggered=True),
                                receiver(page(events=PAIR, releases=1), interruption_triggered=True)),
                               (receiver(), receiver(page(events=PAIR, releases=1), interruption_triggered=None))):
            with self.subTest(initial=initial["interruption_triggered"], final=final["interruption_triggered"]):
                result, checks, record = self.evaluated("canvas_interrupted", session, initial, final)
                self.assertIs(record["observer"]["controlled_interruption"], False)
                self.assertEqual(checks["interruption"], "unverified")
                self.assertEqual(result["status"], "unverified")

    def test_receiver_interruption_without_a_stopped_drag_stays_unverified(self):
        session = self.session("canvas_interrupted")
        session["receipts"] = [{"command": "/owned/cueward window drag --target token", "status": "sent_unverified", "elapsed_ms": 10}]
        final = receiver(page(events=PAIR, releases=1), interruption_triggered=True)
        result, checks, _ = self.evaluated("canvas_interrupted", session, receiver(), final)
        self.assertEqual(checks["interruption"], "unverified")
        self.assertEqual(result["status"], "unverified")

    def test_interrupted_drag_left_held_or_replayed_fails(self):
        held = receiver(page(events=PAIR[:2], dragging=True), interruption_triggered=True)
        result, checks, _ = self.evaluated("canvas_interrupted", self.session("canvas_interrupted"), receiver(), held)
        self.assertEqual(checks["release"], "failed")
        self.assertEqual(result["status"], "failed")
        replayed = self.session("canvas_interrupted", dispatches={"click": 0, "drag": 2})
        released = receiver(page(events=PAIR, releases=1), interruption_triggered=True)
        self.assertEqual(self.evaluated("canvas_interrupted", replayed, receiver(), released)[0]["status"], "failed")


class PointerSessionTests(unittest.TestCase):
    def session(self, action, output="unused"):
        session = task.PointerSession("/mock/cueward", None, PID, WINDOW, output, {}, action=action)
        session.commands.call = Mock(return_value={"status": "sent_unverified"})
        return session

    def pointer(self, action):
        target = token({"kind": "window_input", "window": {"owner_pid": PID, "window_id": WINDOW}})
        return ["window", action, "--target", target] + (["--count", "1"] if action == "click" else [])

    def test_uncounted_routes_never_reach_the_cli(self):
        ax = token({"kind": "app_ax", "app": {"pid": PID}})
        for action, arguments in (("click", ["app", "press", "--target", ax]),
                                  ("click", ["app", "set-value", "--target", ax, "--value", "text"]),
                                  ("click", self.pointer("drag")), ("drag", self.pointer("click"))):
            with self.subTest(action=action, arguments=arguments[:2]):
                session = self.session(action)
                response = session.request(arguments)
                self.assertTrue(response["not_dispatched"])
                session.commands.call.assert_not_called()
                self.assertEqual(session.dispatches, {"click": 0, "drag": 0})

    def test_the_task_action_dispatches_once_and_observation_stays_open(self):
        session = self.session("drag")
        self.assertIn("result", session.request(["window", "snapshot", "--id", str(WINDOW)]))
        self.assertIn("result", session.request(["app", "inspect", "--pid", str(PID)]))
        self.assertIn("result", session.request(self.pointer("drag")))
        self.assertTrue(session.request(self.pointer("drag"))["not_dispatched"])
        self.assertEqual(session.dispatches, {"click": 0, "drag": 1})

    def test_scope_stays_with_the_owned_receiver(self):
        with self.assertRaises(ScopeError):
            self.session("click").invoke(["window", "snapshot", "--id", str(WINDOW + 1)])

    def test_only_images_reported_for_the_owned_window_are_removed(self):
        with tempfile.TemporaryDirectory() as folder:
            owned, foreign = Path(folder) / "owned.png", Path(folder) / "foreign.png"
            owned.write_bytes(b"png")
            foreign.write_bytes(b"png")
            session = self.session("click")
            session.records = [
                {"arguments": ["window", "snapshot"], "result": {"window": {"owner_pid": PID}, "screenshot": {"path": str(owned)}}},
                {"arguments": ["window", "snapshot"], "result": {"window": {"owner_pid": 201}, "screenshot": {"path": str(foreign)}}},
                {"arguments": ["window", "snapshot"], "error": "capture failed"},
                {"arguments": ["app", "inspect"], "result": {"screenshot": {"path": str(foreign)}}}]
            with patch.object(task.AgentSession, "serve_socket", side_effect=RuntimeError("transport failed")):
                with self.assertRaises(RuntimeError):
                    session.serve_socket(Path(folder) / "a.sock")
            self.assertFalse(owned.exists())
            self.assertTrue(foreign.exists())


class ReceiverReadTests(unittest.TestCase):
    def test_setup_reserves_once_and_refuses_a_linked_parent(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            _, goal, output = task.task_setup(root, "canvas_drag")
            self.assertEqual(goal["task_id"], "canvas_drag")
            self.assertEqual(output, root / "operator/canvas_drag")
            with self.assertRaises(FileExistsError):
                task.task_setup(root, "canvas_drag")
            with self.assertRaisesRegex(ValueError, "supported"):
                task.task_setup(root, "calculation")
            other = prepare(parent)
            elsewhere = Path(parent) / "metadata"
            elsewhere.mkdir()
            (other / "operator").symlink_to(elsewhere, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "unlinked"):
                task.task_setup(other, "canvas_drag")
            self.assertEqual(list(elsewhere.iterdir()), [])

    def test_only_the_interrupted_task_opts_into_the_title_change(self):
        for task_id, mode in (("webview_first_click", []), ("canvas_drag", []), ("canvas_interrupted", ["interrupt"])):
            with self.subTest(task_id=task_id), tempfile.TemporaryDirectory() as folder:
                with patch("webview_agent_task.subprocess.Popen") as popen:
                    task.launch_webview("/owned/WebViewFixture", Path(folder) / "receiver.json", task_id)
                self.assertEqual(popen.call_args.args[0][4:], mode)
                self.assertEqual(popen.call_args.args[0][3], "900")

    def test_initial_read_waits_for_the_page_and_gives_up_on_a_page_that_never_loads(self):
        states = [receiver({}), receiver({"events": []}), receiver()]
        with patch.object(task, "receiver_state", side_effect=states) as read:
            self.assertEqual(task.loaded_state("state", Mock()), states[2])
        self.assertEqual(read.call_count, 3)
        with patch.object(task, "receiver_state", return_value=receiver({})), \
                self.assertRaisesRegex(RuntimeError, "did not load"):
            task.loaded_state("state", Mock(), timeout=0.05)

    def test_hover_events_block_only_the_canvas_tasks(self):
        hovered = page(events=[{"type": "mousemove"}])
        self.assertTrue(task.fresh_page("webview_first_click", hovered))
        self.assertFalse(task.fresh_page("canvas_drag", hovered))
        self.assertFalse(task.fresh_page("canvas_interrupted", hovered))
        self.assertFalse(task.fresh_page("webview_first_click", page(clicks=1)))
        self.assertTrue(task.fresh_page("canvas_drag", page()))

    def test_final_read_requires_page_state_newer_than_the_observer(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(task, "SETTLE_SECONDS", 0):
            path = Path(folder) / "receiver.json"
            write_json(path, receiver(dom_updates=5))
            for updates, fresh in ((6, True), (5, False), (None, False)):
                with self.subTest(updates=updates), patch.object(task, "receiver_state", return_value=receiver(dom_updates=updates)) as read:
                    if fresh:
                        self.assertEqual(task.final_state(path, Mock(), WINDOW)["dom_updates"], updates)
                        self.assertTrue(read.call_args.kwargs["fresh"])
                        self.assertEqual(read.call_args.kwargs["expected_window_id"], WINDOW)
                        self.assertIs(read.call_args.kwargs["require_inactive"], False)
                    else:
                        with self.assertRaisesRegex(RuntimeError, "stale"):
                            task.final_state(path, Mock(), WINDOW)


class RunnerTests(unittest.TestCase):
    def run_task(self, root, task_id, *, initial=None, final=None, fail_at=None, events=None):
        """Replace only native launch, observer and Agent transport boundaries."""
        events = [] if events is None else events
        process = Mock(pid=PID)
        process.poll.return_value = 0
        initial = receiver() if initial is None else initial
        output = root / "operator" / task_id

        class SimulatedTransport:
            def __init__(self, cli, observer, pid, window_id, path, metadata, action):
                self.path, self.metadata = path, metadata
                events.append(("session", action))

            def serve_socket(self, socket, duration):
                scope = json.loads((output / "owned-scope.json").read_text())
                events.append(("serve", scope["pid"], scope["window_id"]))
                write_json(self.path, self.metadata | {"receiver_pids": [PID], "records": [{"result": "raw"}],
                           "receipts": [{"command": "/owned/cueward window click", "status": "sent_unverified", "elapsed_ms": 10}],
                           "dispatches": {"click": 1, "drag": 0},
                           "execution": {"start_ms": 0, "end_ms": 100, "samples": [
                               {"ms": ms, "frontmost_pid": 100, "pointer": [12, 34], "visible_spaces": {"display-1": "space-1"},
                                "target_active": {str(PID): False}} for ms in (0, 50, 100)]}})
                if fail_at == "transport":
                    raise RuntimeError("transport failed after dispatch")

        observer = Mock()
        observer.__enter__ = Mock(return_value=observer)
        observer.__exit__ = Mock(side_effect=lambda *_: events.append(("observer_closed",)))

        def load_page(*_):
            events.append(("initial",))
            if fail_at == "page":
                raise RuntimeError("WebView page did not load")
            return initial

        def read_final(*_):
            events.append(("final",))
            if fail_at == "final":
                raise RuntimeError("page stopped reporting")
            return final or receiver(page(clicks=1), observation_sequence=9, fresh_after_sequence=8)

        options = dict(write_report=runner.write_report, record_failure=runner.record_failure, cleanup_receiver=runner.cleanup_receiver)
        with patch.object(task, "compile_webview_fixture", return_value=output / "WebViewFixture"), \
                patch.object(task, "compile_observer"), patch.object(task, "launch_webview", return_value=process) as launch, \
                patch.object(task, "loaded_state", side_effect=load_page), patch.object(task, "final_state", side_effect=read_final), \
                patch.object(task, "DesktopObserver", return_value=observer), patch.object(task, "PointerSession", SimulatedTransport), \
                patch.object(runner, "stop_receiver") as stop:
            try:
                task.run_webview_task(Path("/owned/cueward"), root, task_id, root / "owned.sock", **options)
            finally:
                events.append(("stopped", stop.call_count, launch.call_args.args[2] if launch.called else None))
        return events

    def test_initial_precedes_the_agent_and_final_follows_the_closed_observer(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            events = self.run_task(root, "webview_first_click")
            self.assertEqual([e[0] for e in events], ["initial", "session", "serve", "observer_closed", "final", "stopped"])
            self.assertEqual(events[1], ("session", "click"))
            self.assertEqual(events[2], ("serve", PID, WINDOW))
            self.assertEqual(events[-1][1:3], (1, "webview_first_click"))
            output = root / "operator/webview_first_click"
            scope = json.loads((output / "owned-scope.json").read_text())
            self.assertEqual(set(scope["goal"]), {"task_id", "family", "goal"})
            self.assertTrue(json.loads((output / "cleanup.json").read_text())["owned_process_stopped"])
            report = json.loads((root / "native-agent-report.json").read_text())
            self.assertEqual(next(t for t in report["tasks"] if t["task_id"] == "webview_first_click")["status"], "passed", report)
            self.assertEqual(report["denominator"], 8)

    def test_used_page_is_refused_before_any_agent_gets_scope(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            events = []
            with self.assertRaisesRegex(RuntimeError, "fresh page"):
                self.run_task(root, "canvas_drag", initial=receiver(page(events=[{"type": "mousemove"}])), events=events)
            self.assertNotIn("serve", [e[0] for e in events])
            self.assertFalse((root / "operator/canvas_drag/owned-scope.json").exists())
            self.assertEqual(events[-1][1], 1)
            report = json.loads((root / "native-agent-report.json").read_text())
            self.assertEqual(next(t for t in report["tasks"] if t["task_id"] == "canvas_drag")["status"], "blocked")

    def test_page_that_never_loads_is_blocked_and_still_cleaned_up(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            with self.assertRaisesRegex(RuntimeError, "did not load"):
                self.run_task(root, "webview_first_click", fail_at="page")
            aggregate = json.loads((root / "native-agent-evidence.json").read_text())
            record = aggregate["tasks"]["webview_first_click"]
            self.assertIs(record["attempted"], False)
            self.assertEqual(record["prerequisite"], "fixture_unavailable")
            self.assertTrue(json.loads((root / "operator/webview_first_click/cleanup.json").read_text())["owned_process_stopped"])

    def test_started_attempt_survives_without_a_fabricated_receiver(self):
        for failure in ("transport", "final"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as parent:
                root = prepare(parent)
                with self.assertRaises(RuntimeError):
                    self.run_task(root, "webview_first_click", fail_at=failure)
                record = json.loads((root / "native-agent-evidence.json").read_text())["tasks"]["webview_first_click"]
                self.assertIs(record["attempted"], True)
                self.assertNotIn("observer", record)
                self.assertNotIn("records", record)
                report = json.loads((root / "native-agent-report.json").read_text())
                self.assertEqual(next(t for t in report["tasks"] if t["task_id"] == "webview_first_click")["status"], "unverified")

    def test_entry_runner_accepts_and_routes_the_pointer_tasks(self):
        for task_id in task.TASKS:
            with self.subTest(task_id=task_id), patch.object(runner, "run_webview_task") as run, \
                    patch.object(runner.subprocess, "run"), \
                    patch.object(sys, "argv", ["native-agent-task.py", "--cli", "/owned/cueward", "--run", "/owned/run",
                                               "--task", task_id, "--socket", "/owned/a.sock"]):
                runner.main()
                self.assertEqual(run.call_args.args[2], task_id)
                self.assertIs(run.call_args.kwargs["cleanup_receiver"], runner.cleanup_receiver)


if __name__ == "__main__":
    unittest.main()
