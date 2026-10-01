"""Synthetic regression evidence is not an actual fresh-agent desktop run."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from task_acceptance.evaluate import evaluate, read_scoped
from task_acceptance.setup import CONTENT, EDITED, TASKS, prepare

CLI = Path(__file__).with_name("task-acceptance.py")


class AcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = prepare(self.temp.name)
        self.manifest = json.loads((self.root / "manifest.json").read_text())
        self.run_id = self.manifest["run_id"]
        self.evidence = {"schema": 1, "run_id": self.run_id, "kind": "agent", "tasks": {}}

    def record(self, task="new_document"):
        pid = 200 + list(TASKS).index(task)
        samples = [{"ms": ms, "frontmost_pid": 100, "pointer": [12, 34],
                    "visible_spaces": {"display-1": "space-1"}, "target_active": {str(pid): False}}
                   for ms in range(0, 1001, 100)]
        observer = {"source": "receiver_observer", "run_id": self.run_id,
                    "task_id": task, "pid": pid}
        record = {"run_id": self.run_id, "task_id": task, "attempted": True,
                  "receiver_pids": [pid], "target_space": "space-2", "observer": observer,
                  "receipts": [{"command": "synthetic test receipt", "elapsed_ms": 25, "status": "sent_unverified"}],
                  "execution": {"start_ms": 0, "end_ms": 1000, "samples": samples}}
        if task == "new_document":
            (self.root / "work/new_document/result.txt").write_text(CONTENT, encoding="utf-8")
            observer["save_requests"] = 1
        elif task in ("existing_document", "window_dialog"):
            name = "existing.txt" if task == "existing_document" else "target.txt"
            (self.root / "work" / task / name).write_text(EDITED, encoding="utf-8")
            if task == "window_dialog":
                observer["window_observations"] = [
                    {"phase": phase, "snapshot_id": f"fresh-{phase}",
                     "target_file": str(self.root / "work/window_dialog/target.txt")}
                    for phase in ("initial", "dialog", "resumed")]
        elif task == "cross_app":
            self.record("new_document")
            record["producer_pid"] = 200
            observer.update(file=str(self.root / "work/new_document/result.txt"), content=CONTENT)
        elif task == "calculation":
            (self.root / "work/calculation/result.txt").write_text("69.85\n")
        else:
            observer.update(dispatch_count=1,
                            initial={"clicks": 0, "releases": 0, "events": [], "dragging": False,
                                     "object": {"x": 100, "y": 80}},
                            final={"clicks": 1, "releases": 1, "dragging": False,
                                   "object": {"x": 180, "y": 120},
                                   "events": [{"type": "mousedown", "trusted": True, "buttons": 1},
                                              {"type": "mouseup", "trusted": True, "buttons": 0}]})
            if task == "canvas_interrupted":
                observer["controlled_interruption"] = True
                record["receipts"][0]["status"] = "dispatch_interrupted"
        self.evidence["tasks"][task] = record
        return record

    def result(self, task="new_document"):
        return next(t for t in evaluate(self.root, self.evidence)["tasks"] if t["task_id"] == task)

    def test_reset_is_new_workspace_and_preserves_previous_results(self):
        sentinel = self.root / "result.json"
        sentinel.write_text("retain this audit")
        fresh = prepare(self.temp.name)
        self.assertNotEqual(fresh, self.root)
        self.assertNotEqual(json.loads((fresh / "manifest.json").read_text())["run_id"], self.run_id)
        self.assertEqual(sentinel.read_text(), "retain this audit")
        goals = json.loads((fresh / "agent-goals.json").read_text())
        self.assertEqual(len(goals["tasks"]), 8)
        self.assertNotIn("baseline", goals)

    def test_all_unrun_remain_in_denominator(self):
        report = evaluate(self.root, self.evidence)
        self.assertEqual(report["denominator"], 8)
        self.assertEqual(report["summary"], {"passed": 0, "failed": 0, "blocked": 0, "unverified": 8})

    def test_each_effect_and_artifact_has_an_independent_check(self):
        for task in TASKS:
            self.record(task)
        report = evaluate(self.root, self.evidence)
        self.assertEqual(report["summary"]["passed"], 8, report)
        self.assertEqual(report["support_matrix"]["native_ax"]["passed"], 5)
        self.assertEqual(self.result()["metrics"]["command_count"], 1)
        self.assertEqual(self.result()["metrics"]["tool_wall_ms"], 25)

    def test_blocked_prerequisite_is_not_a_compatibility_failure(self):
        self.evidence["tasks"]["new_document"] = {"run_id": self.run_id, "task_id": "new_document",
            "attempted": False, "prerequisite": "session_locked", "blocked_reason": "desktop locked"}
        self.assertEqual(self.result()["status"], "blocked")
        self.evidence["tasks"]["new_document"]["prerequisite"] = "wrong answer"
        self.assertEqual(self.result()["status"], "unverified")

    def test_wrong_missing_or_oversized_artifact_fails(self):
        self.record()
        path = self.root / "work/new_document/result.txt"
        for content in (b"wrong content", b"x" * (1024 * 1024 + 1)):
            path.write_bytes(content)
            self.assertEqual(self.result()["status"], "failed")
        path.unlink()
        self.assertEqual(self.result()["status"], "failed")
        self.assertEqual(self.result()["metrics"]["command_count"], 1)
        self.assertTrue(self.result()["checks"])

    def test_original_and_bystander_must_be_preserved(self):
        self.record("existing_document")
        (self.root / "work/existing_document/bystander.txt").write_text("changed")
        self.assertEqual(self.result("existing_document")["status"], "failed")
        (self.root / "work/existing_document/bystander.txt").write_text("Preserve this synthetic file.\n")
        self.assertEqual(self.result("existing_document")["status"], "passed")
        (self.root / "seeds/original.txt").write_text("changed")
        self.assertEqual(self.result("existing_document")["status"], "failed")

    def test_unexpected_files_and_links_fail(self):
        self.record("existing_document")
        (self.root / "work/existing_document/unrequested.txt").write_text("extra")
        self.assertEqual(self.result("existing_document")["status"], "failed")
        self.record()
        artifact = self.root / "work/new_document/result.txt"
        artifact.unlink()
        outside = Path(self.temp.name) / "outside.txt"
        outside.write_text(CONTENT)
        artifact.symlink_to(outside)
        self.assertEqual(self.result()["status"], "failed")
        with self.assertRaises(ValueError):
            read_scoped(self.root, "../outside.txt")

    def test_parent_symlink_is_rejected(self):
        self.record()
        folder = self.root / "work/new_document"
        folder.rename(self.root / "outside-work")
        folder.symlink_to(self.root / "outside-work", target_is_directory=True)
        self.assertEqual(self.result()["status"], "failed")

    def test_stale_run_and_observer_evidence_rejected(self):
        record = self.record()
        record["run_id"] = "old-run"
        self.assertEqual(self.result()["status"], "failed")
        record["run_id"] = self.run_id
        record["observer"]["run_id"] = "old-run"
        self.assertEqual(self.result()["status"], "failed")
        with self.assertRaises(ValueError):
            evaluate(self.root, {**self.evidence, "run_id": "old-run"})

    def test_simulated_or_diagnostic_success_never_counts_as_agent_pass(self):
        self.record()
        for kind in ("simulation", "diagnostic"):
            self.evidence["kind"] = kind
            self.assertEqual(self.result()["status"], "unverified")

    def test_missing_receiver_or_trace_is_unverified(self):
        record = self.record()
        record.pop("observer")
        self.assertEqual(self.result()["status"], "unverified")
        record = self.record()
        record["execution"]["samples"] = record["execution"]["samples"][::10]
        self.assertEqual(self.result()["status"], "unverified")

    def test_gaps_missing_channels_and_interval_coverage_are_unverified(self):
        for mutation in (lambda r: r["execution"]["samples"].pop(5),
                         lambda r: r["execution"]["samples"].pop(0),
                         lambda r: r["execution"]["samples"][3].pop("visible_spaces"),
                         lambda r: r["receipts"][0].update(elapsed_ms=float("nan"))):
            record = self.record()
            mutation(record)
            self.assertEqual(self.result()["status"], "unverified")

    def test_transient_interference_fails_even_if_endpoints_match(self):
        for field, value in (("pointer", [13, 34]), ("frontmost_pid", 555),
                             ("visible_spaces", {"display-1": "space-2"}), ("target_active", {"200": True})):
            record = self.record()
            record["execution"]["samples"][5][field] = value
            self.assertEqual(self.result()["status"], "failed", field)

    def test_first_save_must_submit_once_on_inactive_space(self):
        record = self.record()
        record["observer"]["save_requests"] = 2
        self.assertEqual(self.result()["status"], "failed")
        record = self.record()
        record["target_space"] = "space-1"
        self.assertEqual(self.result()["status"], "failed")

    def test_cross_app_requires_distinct_reader_and_exact_received_content(self):
        record = self.record("cross_app")
        record["producer_pid"] = record["observer"]["pid"]
        self.assertEqual(self.result("cross_app")["status"], "failed")
        record["producer_pid"] = 200
        record["observer"]["content"] = "wrong"
        self.assertEqual(self.result("cross_app")["status"], "failed")

    def test_cross_app_must_bind_to_a_verified_producer(self):
        record = self.record("cross_app")
        record["producer_pid"] = 555
        self.assertEqual(self.result("cross_app")["status"], "failed")
        record["producer_pid"] = 200
        self.evidence["tasks"].pop("new_document")
        self.assertEqual(self.result("cross_app")["status"], "unverified")

    def test_partial_receiver_fields_stay_unverified(self):
        for task, field in (("new_document", "save_requests"), ("cross_app", "content"),
                            ("window_dialog", "window_observations"), ("canvas_drag", "final")):
            with self.subTest(task=task):
                record = self.record(task)
                record["observer"].pop(field)
                self.assertEqual(self.result(task)["status"], "unverified")

    def test_window_dialog_requires_fresh_observation_identities(self):
        record = self.record("window_dialog")
        record["observer"]["window_observations"][1]["snapshot_id"] = "fresh-initial"
        self.assertEqual(self.result("window_dialog")["status"], "failed")

    def test_calculation_wrong_rounding_fails(self):
        self.record("calculation")
        (self.root / "work/calculation/result.txt").write_text("69.84\n")
        self.assertEqual(self.result("calculation")["status"], "failed")

    def test_webview_double_click_or_replay_fails(self):
        record = self.record("webview_first_click")
        record["observer"]["final"]["clicks"] = 2
        self.assertEqual(self.result("webview_first_click")["status"], "failed")
        record["observer"]["final"]["clicks"] = 1
        record["observer"]["dispatch_count"] = 2
        self.assertEqual(self.result("webview_first_click")["status"], "failed")

    def test_canvas_events_without_displacement_or_release_fail(self):
        record = self.record("canvas_drag")
        record["observer"]["final"]["object"] = {"x": 100, "y": 80}
        self.assertEqual(self.result("canvas_drag")["status"], "failed")
        record = self.record("canvas_drag")
        record["observer"]["final"]["dragging"] = True
        self.assertEqual(self.result("canvas_drag")["status"], "failed")

    def test_interrupted_drag_needs_independent_interruption_record(self):
        record = self.record("canvas_interrupted")
        record["observer"].pop("controlled_interruption")
        self.assertEqual(self.result("canvas_interrupted")["status"], "unverified")

    def test_unknown_task_cannot_change_denominator(self):
        self.evidence["tasks"]["extra"] = {}
        with self.assertRaises(ValueError):
            evaluate(self.root, self.evidence)

    def test_reused_receiver_cannot_count_as_fresh_task(self):
        self.record("canvas_drag")
        record = self.record("canvas_interrupted")
        record["receiver_pids"] = [206]
        record["observer"]["pid"] = 206
        for sample in record["execution"]["samples"]:
            sample["target_active"] = {"206": False}
        self.assertEqual(self.result("canvas_drag")["status"], "failed")
        self.assertEqual(self.result("canvas_interrupted")["status"], "failed")

    def test_hardlinked_artifact_is_not_accepted_as_owned_disposable_file(self):
        self.record()
        artifact = self.root / "work/new_document/result.txt"
        outside = Path(self.temp.name) / "linked.txt"
        outside.hardlink_to(artifact)
        self.assertEqual(self.result()["status"], "failed")

    def test_cli_prepare_and_unrun_evaluation(self):
        made = subprocess.run([sys.executable, str(CLI), "prepare", "--parent", self.temp.name], capture_output=True, text=True)
        self.assertEqual(made.returncode, 0, made.stderr)
        root = Path(json.loads(made.stdout)["run"])
        output = root / "report.json"
        checked = subprocess.run([sys.executable, str(CLI), "evaluate", "--run", str(root),
                                  "--evidence", str(root / "evidence-template.json"), "--output", str(output)], capture_output=True, text=True)
        self.assertEqual(checked.returncode, 1, checked.stderr)
        self.assertEqual(json.loads(output.read_text())["summary"]["unverified"], 8)

    def test_cli_report_cannot_overwrite_prepared_data_or_evidence(self):
        evidence = self.root / "evidence-template.json"
        for path in (evidence, self.root / "manifest.json", self.root / "work/new_document/result.txt"):
            checked = subprocess.run([sys.executable, str(CLI), "evaluate", "--run", str(self.root),
                                      "--evidence", str(evidence), "--output", str(path)], capture_output=True, text=True)
            self.assertEqual(checked.returncode, 2)
            self.assertIn("must not overwrite", checked.stderr)


if __name__ == "__main__":
    unittest.main()
