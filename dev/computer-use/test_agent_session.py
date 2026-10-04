"""Owned command scope and interactive evidence tests; not fresh-agent acceptance."""
import base64
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest.mock import Mock, patch

from agent_session import AgentSession, validate_command


def token(value):
    return base64.urlsafe_b64encode(json.dumps(value).encode()).decode().rstrip("=")


class AgentSessionTests(unittest.TestCase):
    def test_only_owned_observed_app_and_window_inputs_are_allowed(self):
        ax = token({"kind": "app_ax", "app": {"pid": 100}})
        pointer = token({"kind": "window_input", "window": {"owner_pid": 100, "window_id": 20}})
        for arguments in (["app", "inspect", "--pid", "100"], ["app", "press", "--target", ax],
                          ["app", "set-value", "--target", ax, "--value", "text"],
                          ["window", "snapshot", "--id", "20"],
                          ["window", "drag", "--target", pointer],
                          ["window", "click", "--target", pointer, "--count", "1"]):
            self.assertEqual(validate_command(arguments, 100, 20), arguments)
        forbidden = [["app", "inspect", "--pid", "101"], ["window", "snapshot", "--id", "21"],
                     ["window", "snapshot", "--id", "20", "--output", "/user/file"],
                     ["window", "click", "--target", pointer, "--count", "2"],
                     ["app", "open", "--file", "/user/file"], ["space", "move-window"],
                     ["app", "press", "--target", token({"kind": "app_ax", "app": {"pid": 101}})],
                     ["app", "press", "--target", "invalid!"], ["app", "inspect", "--pid", "100", "--pid", "101"]]
        for arguments in forbidden:
            with self.subTest(arguments=arguments), self.assertRaises(ValueError):
                validate_command(arguments, 100, 20)

    def test_no_pointer_replay_even_after_uncertain_dispatch(self):
        session = AgentSession("/mock/cueward", None, 100, 20, "unused", {})
        session.commands.call = Mock(side_effect=RuntimeError("outcome unknown"))
        pointer = token({"kind": "window_input", "window": {"owner_pid": 100, "window_id": 20}})
        arguments = ["window", "click", "--target", pointer, "--count", "1"]
        self.assertTrue(session.invoke(arguments)["do_not_replay"])
        with self.assertRaisesRegex(ValueError, "no replay"):
            session.invoke(arguments)
        session.commands.call.assert_called_once()
        self.assertEqual(session.dispatches["click"], 1)

    def interactive(self, observer=None, requests=None):
        observer = observer or Mock(mark=Mock(side_effect=["start", "end"]),
                                    execution=Mock(return_value={"start_ms": 10, "end_ms": 20}))
        requests = requests or [["app", "inspect", "--pid", "100"], {"finish": True}]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "evidence.json"
            inputs = Path(directory) / "input.jsonl"
            inputs.write_text("".join(json.dumps(r) + "\n" for r in requests))
            session = AgentSession("/mock/cueward", observer, 100, 20, path, {"task_id": "new_document"})
            session.commands.call = Mock(return_value={"nodes": [{"ref": "w0"}]})
            sink = io.StringIO()
            with inputs.open() as source:
                session.serve(source, sink)
            return json.loads(path.read_text()), session, sink.getvalue()

    def test_interactive_commands_not_runner_selected_steps(self):
        report, session, output = self.interactive()
        session.commands.call.assert_called_once_with("app", "inspect", "--pid", "100")
        self.assertEqual(report["records"][0]["result"]["nodes"][0]["ref"], "w0")
        self.assertEqual(report["execution"]["start_ms"], 10)
        self.assertIn('"ready": true', output)
        self.assertEqual(report["termination_reason"], "finished")
        self.assertEqual(report["request_count"], 2)

    def test_invalid_scoped_command_never_dispatched(self):
        report, session, output = self.interactive(requests=[["app", "inspect", "--pid", "101"], {"finish": True}])
        session.commands.call.assert_not_called()
        self.assertEqual(report["records"], [])
        self.assertIn('"not_dispatched": true', output)

    def test_observer_failure_preserves_records_without_fabricated_execution(self):
        report, _, _ = self.interactive(observer=Mock(mark=Mock(side_effect=["start", RuntimeError("deadline")])))
        self.assertEqual(report["observation_error"], "deadline")
        self.assertEqual(len(report["records"]), 1)
        self.assertNotIn("execution", report)

    def test_real_cross_process_socket_command_and_cleanup(self):
        ready = threading.Event()
        def mark(*args):
            ready.set()
            return "marker"
        with tempfile.TemporaryDirectory(prefix="cwa-") as directory:
            folder = Path(directory)
            observer = Mock(mark=Mock(side_effect=mark), execution=Mock(return_value={"covered": True}))
            session = AgentSession("/mock/cueward", observer, 100, 20, folder / "evidence.json", {})
            session.commands.call = Mock(return_value={"nodes": [{"ref": "w2"}]})
            path = folder / "a.sock"
            worker = threading.Thread(target=session.serve_socket, args=(path, 5))
            worker.start()
            self.assertTrue(ready.wait(3))
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            client = Path(__file__).with_name("agent-command.py")
            result = subprocess.run([sys.executable, str(client), "--socket", str(path), "app", "inspect", "--pid", "100"],
                                    capture_output=True, text=True, check=True, timeout=5)
            subprocess.run([sys.executable, str(client), "--socket", str(path), "--finish"], capture_output=True, check=True, timeout=5)
            worker.join(3)
            self.assertFalse(worker.is_alive())
            self.assertFalse(path.exists())
            rendered = json.loads(result.stdout)["result"]
            self.assertIsInstance(rendered, str)
            body = rendered.split("\n", 1)[1].rsplit("\n</external>", 1)[0]
            self.assertEqual(json.loads(body)["nodes"][0]["ref"], "w2")
            report = json.loads((folder / "evidence.json").read_text())
            self.assertTrue(report["execution"]["covered"])
            session.commands.call.assert_called_once_with("app", "inspect", "--pid", "100")

    def test_existing_socket_path_not_replaced(self):
        with tempfile.TemporaryDirectory(prefix="cwa-") as directory:
            path = Path(directory) / "existing"
            path.write_text("keep")
            session = AgentSession("/mock/cueward", Mock(), 100, 20, Path(directory) / "evidence.json", {})
            with self.assertRaises(OSError):
                session.serve_socket(path)
            self.assertEqual(path.read_text(), "keep")

    def test_post_dispatch_recording_error_is_not_reported_as_no_dispatch(self):
        session = AgentSession("/mock/cueward", None, 100, 20, "unused", {})
        session.recording_started = True
        session.commands.call = Mock(return_value={"status": "sent_unverified"})
        with patch("agent_session.write_json", side_effect=OSError("disk full")):
            with self.assertRaisesRegex(OSError, "disk full"):
                session.request(["app", "inspect", "--pid", "100"])
        session.commands.call.assert_called_once()

    def test_partial_stdin_does_not_prevent_deadline(self):
        with tempfile.TemporaryDirectory() as directory:
            read, write = os.pipe()
            with os.fdopen(read) as source, os.fdopen(write, "w") as writer:
                writer.write('["app",'); writer.flush()
                observer = Mock(mark=Mock(side_effect=["start", "end"]), execution=Mock(return_value={"covered": True}))
                session = AgentSession("/mock/cueward", observer, 100, 20, Path(directory) / "evidence.json", {})
                session.commands.call = Mock()
                worker = threading.Thread(target=session.serve, args=(source, io.StringIO(), 0.1))
                worker.start(); worker.join(2)
                self.assertFalse(worker.is_alive())
                self.assertEqual(session.termination_reason, "deadline")
                session.commands.call.assert_not_called()


if __name__ == "__main__":
    unittest.main()
