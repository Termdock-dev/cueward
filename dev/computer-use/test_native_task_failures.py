"""Incomplete task/cleanup evidence regressions; no desktop receiver is launched."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

from native_agent_task import independent_evidence, task_setup
from task_acceptance.setup import EDITED, ORIGINAL, prepare, write_json

spec = importlib.util.spec_from_file_location("native_runner", Path(__file__).with_name("native-agent-task.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class NativeTaskFailureTests(unittest.TestCase):
    def test_preedited_existing_copy_is_rejected_before_reserving_metadata(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            artifact = root / "work/existing_document/existing.txt"
            artifact.write_text(EDITED)
            with self.assertRaisesRegex(ValueError, "baseline"):
                task_setup(root, "existing_document")
            self.assertFalse((root / "operator").exists())
            self.assertEqual(artifact.read_text(), EDITED)
            self.assertEqual((root / "seeds/original.txt").read_text(), ORIGINAL)

    def test_receiver_io_must_be_observed_not_inferred_from_artifact_or_task_id(self):
        for task in ("existing_document", "calculation"):
            session = {"run_id": "run", "task_id": task, "attempted": True,
                       "receiver_pids": [200], "receipts": [{"command": "actual receipt"}]}
            state = {"pid": 200, "file": "/run/owned.txt", "read_requests": 1,
                     "save_requests": 1, "data_requests": 1, "error": "", "document_edited": False}
            good = independent_evidence(session, state, {"run_id": "run"}, task, 200, Path("/run/owned.txt"))
            self.assertEqual(good["coverage"]["calculation_uses_document_app"], task == "calculation")
            mutations = [{"file": "/run/other.txt"}, {"error": "save failed"}, {"document_edited": True}]
            mutations += [{field: value} for field in ("read_requests", "save_requests", "data_requests")
                          for value in (None, False, True, 0, -1)]
            for mutation in mutations:
                with self.subTest(task=task, mutation=mutation), self.assertRaises(ValueError):
                    independent_evidence(session, state | mutation, {"run_id": "run"}, task, 200,
                                         Path("/run/owned.txt"))

    def run_with_owned_boundaries(self, root, fail_at=None, unregister_exit=0, late_activation=None):
        """Replace only native launch/observer and Agent transport boundaries."""
        process = Mock(pid=200)
        process.poll.return_value = 0
        output = root / "operator/existing_document"
        artifact = root / "work/existing_document/existing.txt"
        initial = {"pid": 200, "window_id": 20, "active": False, "activations": 0,
                   "file": "", "contents": "", "read_requests": 0, "observation_sequence": 1}
        final = initial | {"file": str(artifact), "contents": EDITED, "read_requests": 1,
                           "save_requests": 1, "data_requests": 1, "error": "", "document_edited": False,
                           "observation_sequence": 2, "fresh_after_sequence": 1}
        receipts = [{"command": "synthetic dispatched command", "status": "confirmed", "elapsed_ms": 10}]
        samples = [{"ms": ms, "frontmost_pid": 100, "pointer": [12, 34],
                    "visible_spaces": {"display-1": "space-1"}, "target_active": {"200": False}}
                   for ms in (0, 50, 100)]
        execution = {"start_ms": 0, "end_ms": 100, "samples": samples}

        class SimulatedAgentTransport:
            def __init__(self, cli, observer, pid, window_id, path, metadata):
                self.path, self.metadata = path, metadata

            def serve_socket(self, socket, duration):
                artifact.write_text(EDITED)
                write_json(self.path, self.metadata | {"receiver_pids": [200], "receipts": receipts,
                           "execution": execution, "records": [{"result": "private raw result"}],
                           "observer": self.metadata | {"source": "receiver_observer", "pid": 200},
                           "coverage": {"document_opened_by_agent": True}})
                if fail_at == "transport":
                    raise RuntimeError("transport failed after dispatch")

        context_closed = []
        def read_native_state(path, process, **options):
            if options.get("require_inactive", True):
                return initial
            if fail_at == "final":
                raise RuntimeError("final receiver observation failed")
            if late_activation and context_closed and options.get("fresh"):
                return final | late_activation
            return final

        observer = Mock()
        observer.__enter__ = Mock(return_value=observer)
        observer.__exit__ = Mock(side_effect=lambda *_: context_closed.append(True))
        binary = output / "ExistingDocument.app/Contents/MacOS/ExistingDocument"
        with patch.object(runner, "compile_fixture", return_value=binary), \
                patch.object(runner, "compile_observer"), patch.object(runner, "launch_receiver", return_value=process), \
                patch.object(runner, "receiver_state", side_effect=read_native_state), \
                patch.object(runner, "DesktopObserver", return_value=observer), patch.object(runner, "AgentSession", SimulatedAgentTransport), \
                patch.object(runner, "stop_receiver"), patch.object(runner, "unregister_fixture", return_value=unregister_exit):
            runner.run_task(Path("/owned/cueward"), root, "existing_document", root / "owned.sock")

    def test_attempt_survives_transport_or_final_receiver_failure_without_fabricated_observer(self):
        for failure in ("transport", "final"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as parent:
                root = prepare(parent)
                with self.assertRaises(RuntimeError):
                    self.run_with_owned_boundaries(root, fail_at=failure)
                output = root / "operator/existing_document"
                raw = json.loads((output / "session.json").read_text())
                aggregate = json.loads((root / "native-agent-evidence.json").read_text())
                task = aggregate["tasks"]["existing_document"]
                self.assertTrue(task["attempted"])
                self.assertEqual(task["receipts"], raw["receipts"])
                self.assertEqual(task["execution"], raw["execution"])
                self.assertNotIn("records", task)
                self.assertNotIn("observer", task)
                self.assertNotIn("coverage", task)
                self.assertEqual(task["failure_category"], "partial_observation")
                report = json.loads((root / "native-agent-report.json").read_text())
                result = next(t for t in report["tasks"] if t["task_id"] == "existing_document")
                self.assertEqual(result["status"], "unverified")
                self.assertNotEqual(result["reason"], "not run")
                self.assertEqual(report["denominator"], 8)
                self.assertTrue((output / "runner-error.json").exists())
                self.assertTrue(json.loads((output / "cleanup.json").read_text())["owned_process_stopped"])

    def test_nonzero_bundle_unregistration_is_a_cleanup_failure_not_a_normal_exit(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            with self.assertRaisesRegex(RuntimeError, "cleanup"):
                self.run_with_owned_boundaries(root, unregister_exit=1)
            cleanup = json.loads((root / "operator/existing_document/cleanup.json").read_text())
            self.assertEqual(cleanup["unique_bundle_unregistration_exit"], 1)
            self.assertIn("bundle_cleanup_error", cleanup)
            self.assertTrue(cleanup["owned_process_stopped"])

    def test_unbound_or_unstarted_checkpoint_never_fabricates_attempt_evidence(self):
        for bad in ({"run_id": "another run"}, {"task_id": "calculation"},
                    {"receiver_pids": [201]}, {"attempted": False}):
            with self.subTest(bad=bad), tempfile.TemporaryDirectory() as parent:
                root = prepare(parent)
                manifest, _, _, output = task_setup(root, "existing_document")
                session = {"run_id": manifest["run_id"], "task_id": "existing_document",
                           "receiver_pids": [200], "attempted": True} | bad
                write_json(output / "session.json", session)
                runner.record_failure(root, output, manifest, "existing_document", Mock(pid=200), RuntimeError("failure"))
                self.assertFalse((root / "native-agent-evidence.json").exists())
                self.assertTrue((output / "failure-evidence-error.json").exists())
                self.assertEqual(json.loads((output / "session.json").read_text()), session)

    def test_cleanup_exceptions_still_record_and_attempt_other_owned_cleanup(self):
        for failed_operation in ("stop_receiver", "unregister_fixture"):
            with self.subTest(operation=failed_operation), tempfile.TemporaryDirectory() as parent:
                output = Path(parent)
                process = Mock(pid=200)
                process.poll.return_value = 0
                binary = output / "ExistingDocument.app/Contents/MacOS/ExistingDocument"
                stop_error = OSError("owned process cleanup failed") if failed_operation == "stop_receiver" else None
                bundle_error = subprocess.TimeoutExpired("owned bundle cleanup", 20) if failed_operation == "unregister_fixture" else None
                with patch.object(runner, "stop_receiver", side_effect=stop_error), \
                        patch.object(runner, "unregister_fixture", side_effect=bundle_error, return_value=0) as unregister:
                    with self.assertRaisesRegex(RuntimeError, "cleanup"):
                        runner.cleanup_receiver(process, binary, output, "existing_document")
                    unregister.assert_called_once()
                cleanup = json.loads((output / "cleanup.json").read_text())
                self.assertIn("process_cleanup_error" if stop_error else "bundle_cleanup_error", cleanup)
                self.assertTrue(cleanup["owned_process_stopped"])


if __name__ == "__main__":
    unittest.main()
