"""New-document and cross-app runner regressions; no desktop receiver is launched."""
import base64
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

from agent_session import ScopeError, validate_command
import document_agent_task as documents
from native_agent_task import unregister_fixture
from task_acceptance.evaluate import evaluate
from task_acceptance.setup import CONTENT, prepare, write_json

spec = importlib.util.spec_from_file_location("native_runner", Path(__file__).with_name("native-agent-task.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)

WRITER, READER = 200, 300
MOVED = {"status": "confirmed", "foreground_changed": False, "visible_spaces_changed": False, "after_spaces": [6]}


def space_cli(moved=None, spaces=({"id": 1, "type": 0, "is_visible": True}, {"id": 6, "type": 0, "is_visible": False})):
    """Answer setup commands and keep them, so a test can prove a move was not requested."""
    calls = []

    def call(_, *arguments):
        calls.append(arguments)
        if arguments[:2] == ("space", "list"):
            return {"displays": [{"spaces": list(spaces)}]}
        if arguments[:2] == ("space", "window"):
            return {"move_target": "observed-move-target"}
        return MOVED | (moved or {})
    return call, calls


def execution(pid):
    samples = [{"ms": ms, "frontmost_pid": 100, "pointer": [12, 34], "visible_spaces": {"display-1": "1"},
                "target_active": {str(pid): False}} for ms in (0, 50, 100)]
    return {"start_ms": 0, "end_ms": 100, "samples": samples}


def receiver(task, pid):
    """Return untouched initial and clean final states; a test overrides the final one."""
    common = {"pid": pid, "window_id": 20, "active": False, "activations": 0, "observation_sequence": 1}
    if task == "new_document":
        initial = common | {"contents": "", "save_requests": 0, "file_created": False, "panel_window_id": 0}
        return initial, initial | {"save_requests": 1, "contents": CONTENT}
    initial = common | {"file": "", "content": "", "read_requests": 0, "error": ""}
    return initial, initial | {"read_requests": 1, "content": CONTENT}


class DocumentTaskTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = prepare(temporary.name)
        self.artifact = self.root / "work/new_document/result.txt"
        self.launched = []
        self.stopped = []

    def run_task(self, task, final=None, *, saved=True, cli=None, fail_at=None, initial=None):
        """Replace only the native launch, observer and Agent transport boundaries."""
        pid = WRITER if task == "new_document" else READER
        fresh, clean = receiver(task, pid)
        initial = fresh | (initial or {})
        opened = {"file": str(self.artifact)} if task == "cross_app" else {}
        final = clean | opened | {"observation_sequence": 2, "fresh_after_sequence": 1} | (final or {})
        process = Mock(pid=pid)
        process.poll.return_value = 0
        artifact, launched = self.artifact, self.launched

        def launch(binary, *_):
            launched.append(binary)
            return process

        class Transport:
            def __init__(self, cli, observer, pid, window_id, path, metadata):
                self.pid, self.path, self.metadata = pid, path, metadata

            def serve_socket(self, socket, duration):
                if saved and task == "new_document":
                    artifact.write_text(CONTENT, encoding="utf-8")
                write_json(self.path, self.metadata | {
                    "receiver_pids": [self.pid], "execution": execution(self.pid), "records": [{"result": "private"}],
                    "receipts": [{"command": "synthetic command", "status": "confirmed", "elapsed_ms": 10}]})
                if fail_at == "transport":
                    raise RuntimeError("transport failed after dispatch")

        def state(path, process, **options):
            return final if options.get("fresh") else initial

        observer = Mock()
        observer.__enter__ = Mock(return_value=observer)
        observer.__exit__ = Mock(return_value=False)
        reader = self.root / "operator/cross_app/DocumentReader.app/Contents/MacOS/DocumentReader"
        with patch.object(documents, "compile_document_fixture", return_value=Path("/owned/NewDocument")), \
                patch.object(documents, "compile_reader", return_value=reader), patch.object(documents, "compile_observer"), \
                patch.object(documents, "launch_document", side_effect=launch), \
                patch.object(documents, "launch_receiver", side_effect=launch), \
                patch.object(documents, "receiver_state", side_effect=state), \
                patch.object(documents, "DesktopObserver", return_value=observer), \
                patch.object(documents, "AgentSession", Transport), \
                patch.object(documents, "cli_json", cli or space_cli()[0]), \
                patch.object(runner, "stop_receiver", side_effect=self.stopped.append), \
                patch.object(runner, "unregister_fixture", return_value=0) as unregister:
            try:
                runner.run_task(Path("/owned/cueward"), self.root, task, self.root / "owned.sock")
            finally:
                self.unregistered = [call.args[0] for call in unregister.call_args_list]

    def evidence(self):
        return json.loads((self.root / "native-agent-evidence.json").read_text())

    def result(self, task):
        report = evaluate(self.root, self.evidence())
        return next(t for t in report["tasks"] if t["task_id"] == task)

    def test_setup_reserves_once_and_refuses_an_existing_destination(self):
        manifest, goal, artifact, output = documents.document_setup(self.root, "new_document")
        self.assertEqual(goal["content"], CONTENT)
        self.assertEqual(artifact, self.artifact)
        self.assertEqual(output, self.root / "operator/new_document")
        self.assertFalse(artifact.exists())
        with self.assertRaises(FileExistsError):
            documents.document_setup(self.root, "new_document")
        other = prepare(self.root.parent)
        (other / "work/new_document/result.txt").write_text(CONTENT)
        with self.assertRaisesRegex(ValueError, "no existing file"):
            documents.document_setup(other, "new_document")
        self.assertFalse((other / "operator").exists())

    def test_move_refuses_without_an_inactive_space_and_requests_no_move(self):
        for spaces in (({"id": 1, "type": 0, "is_visible": True},), ({"id": 9, "type": 4, "is_visible": False},)):
            with self.subTest(spaces=spaces):
                call, calls = space_cli(spaces=spaces)
                with patch.object(documents, "cli_json", call), self.assertRaisesRegex(RuntimeError, "no existing inactive Space"):
                    documents.move_to_inactive_space("/owned/cueward", 20, self.root / "space-move.json")
                self.assertEqual(calls, [("space", "list")])

    def test_move_refuses_any_change_to_foreground_or_visible_spaces(self):
        for changed in ({"foreground_changed": True}, {"visible_spaces_changed": True}, {"status": "sent_unverified"},
                        {"foreground_changed": None}, {"visible_spaces_changed": None}, {"after_spaces": [1]}):
            with self.subTest(changed=changed):
                with patch.object(documents, "cli_json", space_cli(changed)[0]), self.assertRaisesRegex(RuntimeError, "did not preserve"):
                    documents.move_to_inactive_space("/owned/cueward", 20, self.root / "space-move.json")

    def test_move_only_uses_commands_for_the_owned_window(self):
        call, calls = space_cli()
        with patch.object(documents, "cli_json", call):
            self.assertEqual(documents.move_to_inactive_space("/owned/cueward", 20, self.root / "space-move.json"), "6")
        self.assertEqual(calls, [("space", "list"), ("space", "window", "--id", 20),
                                 ("space", "move-window", "--target", "observed-move-target", "--space", 6)])

    def test_refused_setup_is_blocked_and_never_serves_an_agent(self):
        call, calls = space_cli(spaces=({"id": 1, "type": 0, "is_visible": True},))
        with self.assertRaisesRegex(RuntimeError, "no existing inactive Space"):
            self.run_task("new_document", cli=call)
        self.assertFalse((self.root / "operator/new_document/owned-scope.json").exists())
        self.assertFalse((self.root / "operator/new_document/session.json").exists())
        self.assertEqual(self.result("new_document")["status"], "blocked")
        self.assertEqual(len(self.stopped), 1)
        self.assertEqual(self.unregistered, [])

    def test_touched_receiver_never_reaches_the_space_move(self):
        for changed in ({"contents": "typed"}, {"save_requests": 1}, {"file_created": True}):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as parent:
                self.root = prepare(parent)
                call, calls = space_cli()
                with self.assertRaisesRegex(RuntimeError, "empty unsaved"):
                    self.run_task("new_document", cli=call, initial=changed)
                self.assertEqual(calls, [])

    def test_no_artifact_is_a_complete_attempt_and_not_a_crash(self):
        self.run_task("new_document", saved=False, final={"panel_window_id": 77})
        record = self.evidence()["tasks"]["new_document"]
        self.assertIs(record["attempted"], True)
        self.assertEqual(record["receiver_pids"], [WRITER])
        self.assertEqual(record["target_space"], "6")
        self.assertEqual(record["failure_category"], "missing_artifact")
        self.assertEqual(record["observer"]["save_requests"], 1)
        self.assertEqual(record["observer"]["panel_window_id"], 77)
        self.assertEqual(record["receipts"][0]["command"], "synthetic command")
        self.assertNotIn("records", record)
        self.assertTrue(record["coverage"]["first_save_panel"])
        result = self.result("new_document")
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["reported_failure_category"], "missing_artifact")
        self.assertEqual(next(c for c in result["checks"] if c["name"] == "artifact")["status"], "failed")
        self.assertEqual(next(c for c in result["checks"] if c["name"] == "save_once")["status"], "passed")
        self.assertEqual(next(c for c in result["checks"] if c["name"] == "final_receiver_inactive")["status"], "passed")
        self.assertEqual(json.loads((self.root / "native-agent-report.json").read_text())["denominator"], 8)
        self.assertEqual(len(self.stopped), 1)

    def test_saved_artifact_can_pass_and_needs_no_bundle_cleanup(self):
        self.run_task("new_document")
        self.assertEqual(self.result("new_document")["status"], "passed", self.result("new_document"))
        self.assertNotIn("failure_category", self.evidence()["tasks"]["new_document"])
        self.assertEqual(self.unregistered, [])
        cleanup = json.loads((self.root / "operator/new_document/cleanup.json").read_text())
        self.assertTrue(cleanup["owned_process_stopped"])

    def test_receiver_that_never_reported_saves_stays_unverified(self):
        final = dict(receiver("new_document", WRITER)[1])
        _, _, artifact, _ = documents.document_setup(self.root, "new_document")
        final.pop("save_requests")
        session = {"run_id": "run", "task_id": "new_document", "receiver_pids": [WRITER]}
        record = documents.new_document_evidence(session, final, {"run_id": "run"}, WRITER, artifact, "6")
        self.assertNotIn("save_requests", record["observer"])
        with self.assertRaisesRegex(ValueError, "identity"):
            documents.new_document_evidence(session, final | {"pid": 999}, {"run_id": "run"}, WRITER, artifact, "6")

    def test_late_activation_fails_and_unfresh_final_state_is_unverified(self):
        for task in ("new_document", "cross_app"):
            for changed, expected in (({"active": True}, "failed"), ({"activations": 1}, "failed"),
                                      ({"observation_sequence": 1}, "unverified"), ({"fresh_after_sequence": 0}, "unverified")):
                with self.subTest(task=task, changed=changed), tempfile.TemporaryDirectory() as parent:
                    self.root = prepare(parent)
                    self.artifact = self.root / "work/new_document/result.txt"
                    self.run_task("new_document", changed if task == "new_document" else None)
                    if task == "cross_app":
                        self.run_task("cross_app", changed)
                    result = self.result(task)
                    self.assertEqual(result["status"], expected, result)
                    self.assertEqual(next(c for c in result["checks"] if c["name"] == "final_receiver_inactive")["status"], expected)

    def test_transport_failure_after_start_keeps_the_attempt(self):
        with self.assertRaisesRegex(RuntimeError, "transport failed"):
            self.run_task("new_document", saved=False, fail_at="transport")
        record = self.evidence()["tasks"]["new_document"]
        self.assertIs(record["attempted"], True)
        self.assertEqual(record["failure_category"], "partial_observation")
        self.assertNotIn("observer", record)
        self.assertEqual(len(self.stopped), 1)

    def test_cross_app_stops_before_any_launch_without_a_passed_producer(self):
        cases = {"no evidence": lambda: None,
                 "not started": lambda: write_json(self.root / "native-agent-evidence.json", {
                     "schema": 1, "run_id": json.loads((self.root / "manifest.json").read_text())["run_id"],
                     "kind": "agent", "tasks": {}}),
                 "failed, no artifact": lambda: self.run_task("new_document", saved=False),
                 "failed, two saves": lambda: self.run_task("new_document", {"save_requests": 2})}
        for label, arrange in cases.items():
            with self.subTest(label), tempfile.TemporaryDirectory() as parent:
                self.root = prepare(parent)
                self.artifact = self.root / "work/new_document/result.txt"
                arrange()
                before = (self.root / "native-agent-evidence.json").read_bytes() if label != "no evidence" else None
                self.launched.clear()
                with self.assertRaises(SystemExit) as stopped:
                    self.run_task("cross_app")
                self.assertIn("cross_app not started", str(stopped.exception.code))
                self.assertIn("Nothing was launched", str(stopped.exception.code))
                self.assertEqual(self.launched, [])
                self.assertFalse((self.root / "operator/cross_app").exists())
                if before is not None:
                    self.assertEqual((self.root / "native-agent-evidence.json").read_bytes(), before)
                evidence = {"schema": 1, "run_id": json.loads((self.root / "manifest.json").read_text())["run_id"],
                            "kind": "agent", "tasks": {}}
                if before is not None:
                    evidence = self.evidence()
                result = next(t for t in evaluate(self.root, evidence)["tasks"] if t["task_id"] == "cross_app")
                self.assertEqual(result["status"], "unverified")

    def test_cross_app_uses_the_producer_from_this_runs_evidence(self):
        self.run_task("new_document")
        self.assertEqual(documents.passed_producer(self.root), WRITER)
        self.run_task("cross_app")
        record = self.evidence()["tasks"]["cross_app"]
        self.assertEqual(record["producer_pid"], WRITER)
        self.assertEqual(record["receiver_pids"], [READER])
        self.assertEqual(record["observer"]["content"], CONTENT)
        self.assertEqual(self.result("cross_app")["status"], "passed", self.result("cross_app"))
        self.assertEqual(self.launched[-1], self.unregistered[0])

    def test_invented_or_mismatched_producer_is_rejected(self):
        session = {"run_id": "run", "task_id": "cross_app", "receiver_pids": [READER]}
        final = receiver("cross_app", READER)[1]
        for invented in (None, 0, -1, True, str(WRITER), READER):
            with self.subTest(invented=invented), self.assertRaisesRegex(ValueError, "distinct"):
                documents.reader_evidence(session, final, {"run_id": "run"}, READER, invented)
        self.run_task("new_document")
        self.run_task("cross_app")
        evidence = self.evidence()
        evidence["tasks"]["cross_app"]["producer_pid"] = WRITER + 1
        write_json(self.root / "native-agent-evidence.json", evidence)
        self.assertEqual(self.result("cross_app")["status"], "failed")
        evidence["tasks"]["cross_app"].pop("producer_pid")
        write_json(self.root / "native-agent-evidence.json", evidence)
        self.assertEqual(self.result("cross_app")["status"], "unverified")

    def test_reader_that_opened_nothing_is_a_failed_attempt(self):
        self.run_task("new_document")
        self.run_task("cross_app", {"file": "", "content": "", "read_requests": 0})
        self.assertFalse(self.evidence()["tasks"]["cross_app"]["coverage"]["document_opened_by_agent"])
        self.assertEqual(self.result("cross_app")["status"], "failed")

    def test_reader_cannot_keep_a_changed_artifact_passing(self):
        self.run_task("new_document")
        self.run_task("cross_app")
        self.artifact.write_text("changed by the reader")
        self.assertEqual(self.result("cross_app")["status"], "unverified")
        self.assertEqual(next(c for c in self.result("cross_app")["checks"] if c["name"] == "artifact")["status"], "failed")

    def test_panel_target_with_another_receiver_passes_and_a_foreign_app_does_not(self):
        def token(pid):
            value = {"kind": "app_ax", "app": {"pid": pid}, "ref": "w0.1.2"}
            return base64.urlsafe_b64encode(json.dumps(value).encode()).rstrip(b"=").decode()

        panel_control = {"receiver_pid": 555, "target": token(WRITER)}
        for action in (["press"], ["set-value", "--value", "result.txt"]):
            command = ["app", action[0], "--target", panel_control["target"], *action[1:]]
            self.assertEqual(validate_command(command, WRITER, 20), command)
            with self.assertRaisesRegex(ScopeError, "outside owned scope"):
                validate_command(["app", action[0], "--target", token(555), *action[1:]], WRITER, 20)
        with self.assertRaisesRegex(ScopeError, "outside owned scope"):
            validate_command(["app", "inspect", "--pid", "555"], WRITER, 20)

    def test_reader_bundle_cleanup_is_allowed_and_other_paths_are_not(self):
        binary = Path("/owned/DocumentReader.app/Contents/MacOS/DocumentReader")
        with patch("native_agent_task.subprocess.run", return_value=Mock(returncode=0)) as run:
            self.assertEqual(unregister_fixture(binary), 0)
            self.assertEqual(run.call_args.args[0][1:], ["-u", "/owned/DocumentReader.app"])
            for other in ("/owned/NewDocument.app/Contents/MacOS/NewDocument",
                          "/owned/DocumentReader.app/Contents/MacOS/Other",
                          "/owned/Other.app/Contents/MacOS/DocumentReader"):
                with self.assertRaises(ValueError):
                    unregister_fixture(other)
            run.assert_called_once()

    def test_entry_point_accepts_both_tasks_and_dispatches_them(self):
        for task in documents.DOCUMENT_TASKS:
            with self.subTest(task=task), patch.object(runner.subprocess, "run"), \
                    patch.object(runner, "run_task") as run_task, \
                    patch.object(sys, "argv", ["native-agent-task.py", "--cli", "/owned/cueward", "--run", str(self.root),
                                               "--task", task, "--socket", "/owned.sock"]):
                runner.main()
                self.assertEqual(run_task.call_args.args[2], task)


if __name__ == "__main__":
    unittest.main()
