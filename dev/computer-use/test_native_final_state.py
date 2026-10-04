"""Final activation/freshness regressions with synthetic files, never desktop actions."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import test_native_task_failures as boundaries
from native_agent_task import independent_evidence, receiver_state, task_setup
from task_acceptance.evaluate import evaluate
from task_acceptance.setup import EDITED, prepare, write_json


class NativeFinalStateTests(unittest.TestCase):
    def task_evidence(self, root, task, changes=None):
        manifest, _, artifact, _ = task_setup(root, task)
        artifact.write_text(EDITED if task == "existing_document" else "69.85\n")
        session = {"run_id": manifest["run_id"], "task_id": task, "attempted": True,
                   "receiver_pids": [200],
                   "receipts": [{"command": "synthetic command", "status": "confirmed", "elapsed_ms": 10}],
                   "execution": {"start_ms": 0, "end_ms": 100,
                                 "samples": [{"ms": ms, "frontmost_pid": 100, "pointer": [12, 34],
                                              "visible_spaces": {"display-1": "space-1"},
                                              "target_active": {"200": False}} for ms in (0, 50, 100)]}}
        state = {"pid": 200, "window_id": 20, "file": str(artifact), "read_requests": 1,
                 "save_requests": 1, "data_requests": 1, "document_edited": False, "error": "",
                 "active": False, "activations": 0, "observation_sequence": 2, "fresh_after_sequence": 1}
        record = independent_evidence(session, state | (changes or {}), manifest, task, 200, artifact)
        return {"schema": 1, "run_id": manifest["run_id"], "kind": "agent", "tasks": {task: record}}

    def test_late_or_transient_activation_fails_despite_correct_bytes_and_stationary_trace(self):
        for task in ("existing_document", "calculation"):
            for changed in ({"active": True, "activations": 0}, {"active": True, "activations": 1},
                            {"active": False, "activations": 1}):
                with self.subTest(task=task, changed=changed), tempfile.TemporaryDirectory() as parent:
                    root = prepare(parent)
                    evidence = self.task_evidence(root, task, changed)
                    result = next(t for t in evaluate(root, evidence)["tasks"] if t["task_id"] == task)
                    self.assertEqual(result["status"], "failed")
                    self.assertEqual(next(c for c in result["checks"] if c["name"] == "artifact")["status"], "passed")
                    self.assertEqual(next(c for c in result["checks"] if c["name"] == "interference_trace")["status"], "passed")
                    observer = evidence["tasks"][task]["observer"]
                    self.assertEqual(observer["active"], changed["active"])
                    self.assertEqual(observer["activations"], changed["activations"])

    def test_missing_malformed_or_unfresh_final_activity_cannot_pass(self):
        for task in ("existing_document", "calculation"):
            for changed in ({"active": None}, {"active": 0}, {"activations": None}, {"activations": False},
                            {"activations": -1}, {"activations": 0.0}, {"observation_sequence": None},
                            {"observation_sequence": True}, {"observation_sequence": 1},
                            {"fresh_after_sequence": 0}, {"fresh_after_sequence": None}, {"window_id": None}):
                with self.subTest(task=task, changed=changed), tempfile.TemporaryDirectory() as parent:
                    root = prepare(parent)
                    evidence = self.task_evidence(root, task, changed)
                    result = next(t for t in evaluate(root, evidence)["tasks"] if t["task_id"] == task)
                    self.assertEqual(result["status"], "unverified")

    def test_valid_inactive_fresh_observation_is_retained_and_can_pass(self):
        with tempfile.TemporaryDirectory() as parent:
            root = prepare(parent)
            evidence = self.task_evidence(root, "existing_document")
            result = next(t for t in evaluate(root, evidence)["tasks"] if t["task_id"] == "existing_document")
            self.assertEqual(result["status"], "passed")
            observer = evidence["tasks"]["existing_document"]["observer"]
            self.assertEqual({k: observer[k] for k in ("active", "activations", "window_id", "observation_sequence", "fresh_after_sequence")},
                             {"active": False, "activations": 0, "window_id": 20, "observation_sequence": 2, "fresh_after_sequence": 1})
            # Older identity/disk-only supplied evidence keeps its documented contract.
            for key in ("active", "activations", "observation_sequence", "fresh_after_sequence", "window_id"):
                observer.pop(key)
            self.assertEqual(next(t for t in evaluate(root, evidence)["tasks"] if t["task_id"] == "existing_document")["status"], "passed")

    def test_fresh_read_waits_for_a_new_snapshot_not_a_reread_of_the_same_file(self):
        with tempfile.TemporaryDirectory() as parent:
            path = Path(parent) / "receiver.json"
            process = Mock(pid=200)
            process.poll.return_value = None
            baseline = {"pid": 200, "window_id": 20, "active": False, "activations": 0, "observation_sequence": 10}
            write_json(path, baseline)
            sleeps = []

            def tick(_):
                sleeps.append(True)
                if len(sleeps) == 2:
                    write_json(path, baseline | {"observation_sequence": 11, "activations": 1})

            with patch("native_agent_task.time.sleep", side_effect=tick):
                observed = receiver_state(path, process, require_inactive=False, fresh=True, expected_window_id=20)
            self.assertEqual(len(sleeps), 2)
            self.assertEqual(observed["fresh_after_sequence"], 10)
            self.assertEqual(observed["observation_sequence"], 11)
            self.assertEqual(observed["activations"], 1)

    def test_frozen_invalid_or_rebound_snapshots_fail_instead_of_certifying_freshness(self):
        baseline = {"pid": 200, "window_id": 20, "active": False, "activations": 0, "observation_sequence": 10}
        cases = [("frozen", None)] + [("sequence", {"observation_sequence": value}) for value in (None, True, False, 0, -1, 1.5)]
        cases += [("identity", {"pid": 201, "observation_sequence": 11}),
                  ("identity", {"window_id": 21, "observation_sequence": 11}),
                  ("sequence", {"observation_sequence": 9})]
        for label, mutation in cases:
            with self.subTest(label=label, mutation=mutation), tempfile.TemporaryDirectory() as parent:
                path = Path(parent) / "receiver.json"
                write_json(path, baseline)
                process = Mock(pid=200)
                process.poll.return_value = None
                if mutation is None:
                    with self.assertRaisesRegex(RuntimeError, "fresh"):
                        receiver_state(path, process, timeout=0.02, require_inactive=False, fresh=True, expected_window_id=20)
                else:
                    with patch("native_agent_task.time.sleep", side_effect=lambda _: write_json(path, baseline | mutation)):
                        with self.assertRaises(RuntimeError):
                            receiver_state(path, process, require_inactive=False, fresh=True, expected_window_id=20)

    def test_runner_takes_fresh_final_state_only_after_observer_context_closes(self):
        for changed in ({"active": True, "activations": 0}, {"active": False, "activations": 1}):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as parent:
                root = prepare(parent)
                boundaries.NativeTaskFailureTests().run_with_owned_boundaries(root, late_activation=changed)
                output = root / "operator/existing_document"
                final = json.loads((output / "receiver-final.json").read_text())
                self.assertEqual(final["active"], changed["active"])
                self.assertEqual(final["activations"], changed["activations"])
                report = json.loads((root / "native-agent-report.json").read_text())
                self.assertEqual(next(t for t in report["tasks"] if t["task_id"] == "existing_document")["status"], "failed")
                self.assertTrue(json.loads((output / "cleanup.json").read_text())["owned_process_stopped"])


if __name__ == "__main__":
    unittest.main()
