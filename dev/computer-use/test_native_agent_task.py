"""Setup, scope, evidence and real child cleanup; these are not desktop task passes."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

from native_agent_task import (independent_evidence, merge_evidence, receiver_state, stop_receiver,
                               task_setup, unregister_fixture)
from task_acceptance.setup import ORIGINAL, prepare, write_json


class NativeAgentTaskTests(unittest.TestCase):
    def test_existing_copy_is_not_preedited_and_metadata_stays_outside_workspace(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            manifest, goal, artifact, output = task_setup(root, "existing_document")
            self.assertEqual(artifact.read_text(), ORIGINAL)
            self.assertEqual(goal["artifact"], str(artifact))
            self.assertEqual(output, root / "operator/existing_document")
            self.assertEqual({p.name for p in artifact.parent.iterdir()}, {"existing.txt", "bystander.txt"})
            self.assertEqual((root / "seeds/original.txt").read_text(), ORIGINAL)
            with self.assertRaises(FileExistsError):
                task_setup(root, "existing_document")

    def test_calculation_is_blank_and_existing_artifact_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            _, goal, artifact, _ = task_setup(root, "calculation")
            self.assertIsNone(goal["content"])
            self.assertEqual(artifact.read_bytes(), b"")
            other = prepare(parent)
            existing = other / "work/calculation/result.txt"
            existing.write_text("preserve old evidence")
            with self.assertRaises(FileExistsError):
                task_setup(other, "calculation")
            self.assertEqual(existing.read_text(), "preserve old evidence")

    def test_symlink_workspace_is_rejected_before_creating_a_result_elsewhere(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            elsewhere = Path(parent) / "elsewhere"
            elsewhere.mkdir()
            target = root / "work/calculation"
            target.rmdir()
            target.symlink_to(elsewhere, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "unlinked"):
                task_setup(root, "calculation")
            self.assertFalse((elsewhere / "result.txt").exists())

    def test_symlink_operator_parent_is_rejected_before_writing_metadata(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            elsewhere = Path(parent) / "metadata"
            elsewhere.mkdir()
            (root / "operator").symlink_to(elsewhere, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "unlinked"):
                task_setup(root, "existing_document")
            self.assertEqual(list(elsewhere.iterdir()), [])

    def test_real_child_handle_cleanup_does_not_depend_on_state(self):
        child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
        try:
            stop_receiver(child)
            self.assertIsNotNone(child.poll())
            stop_receiver(child)
        finally:
            if child.poll() is None:
                child.kill(); child.wait()

    def test_bundle_cleanup_is_scoped_and_never_unregisters_other_bundle_paths(self):
        binary = Path("/owned/ExistingDocument.app/Contents/MacOS/ExistingDocument")
        with patch("native_agent_task.subprocess.run", return_value=Mock(returncode=0)) as run:
            self.assertEqual(unregister_fixture(binary), 0)
            self.assertEqual(run.call_args.args[0][1:], ["-u", "/owned/ExistingDocument.app"])
            for other in ("/user/Editor.app/Contents/MacOS/Editor", "/owned/ExistingDocument.app/Contents/MacOS/Other"):
                with self.assertRaises(ValueError):
                    unregister_fixture(other)
            run.assert_called_once()

    def test_receiver_identity_and_activation_are_not_silently_accepted(self):
        child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
        try:
            with tempfile.TemporaryDirectory() as folder:
                path = Path(folder) / "receiver.json"
                state = {"pid": child.pid, "window_id": 20, "active": False, "activations": 0}
                write_json(path, state)
                self.assertEqual(receiver_state(path, child), state)
                write_json(path, state | {"pid": child.pid + 1})
                with self.assertRaisesRegex(RuntimeError, "identity"):
                    receiver_state(path, child)
                write_json(path, state | {"active": True, "activations": 1})
                with self.assertRaisesRegex(RuntimeError, "active"):
                    receiver_state(path, child)
                self.assertTrue(receiver_state(path, child, require_inactive=False)["active"])
        finally:
            stop_receiver(child)

    def test_independent_state_keeps_actual_receipts_and_trace_and_rejects_wrong_pid(self):
        session = {"run_id": "run", "task_id": "existing_document", "receiver_pids": [200],
                   "receipts": [{"command": "actual command", "status": "tool_error", "elapsed_ms": 12}],
                   "execution": {"samples": [{"frontmost_pid": 100}]}, "records": [{"result": "raw UI"}]}
        state = {"pid": 200, "file": "/run/existing.txt", "read_requests": 1, "save_requests": 1,
                 "data_requests": 1, "error": "", "document_edited": False}
        evidence = independent_evidence(session, state, {"run_id": "run"}, "existing_document", 200,
                                        Path("/run/existing.txt"))
        self.assertNotIn("records", evidence)
        self.assertEqual(evidence["receipts"], session["receipts"])
        self.assertEqual(evidence["execution"], session["execution"])
        self.assertEqual(evidence["observer"]["source"], "receiver_observer")
        self.assertTrue(evidence["coverage"]["document_opened_by_agent"])
        with self.assertRaisesRegex(ValueError, "identity"):
            independent_evidence(session, state | {"pid": 201}, {"run_id": "run"},
                                 "existing_document", 200, Path("/run/existing.txt"))

    def test_aggregate_retains_other_task_and_refuses_replacing_original_evidence(self):
        with tempfile.TemporaryDirectory() as folder:
            first = {"run_id": "run", "task_id": "existing_document", "receiver_pids": [200]}
            second = {"run_id": "run", "task_id": "calculation", "receiver_pids": [201]}
            path = merge_evidence(folder, first)
            merge_evidence(folder, second)
            self.assertEqual(json.loads(path.read_text())["tasks"],
                             {"existing_document": first, "calculation": second})
            original = path.read_bytes()
            with self.assertRaisesRegex(ValueError, "overwritten"):
                merge_evidence(folder, first)
            self.assertEqual(path.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
