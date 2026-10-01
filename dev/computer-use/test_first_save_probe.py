"""Exercise probe orchestration without launching apps or sending desktop input."""
import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("first_save_probe", Path(__file__).with_name("first-save-probe.py"))
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class FirstSaveTests(unittest.TestCase):
    def scenario(self, document=None, reader_activations=0, opened=None, delayed_panel=False,
                 move_target="observed-token", content=None, panel_controls=None, artifact_missing=False,
                 through_main=False):
        state = {"active": False, "activations": 0, "foreground_changes": 0,
                 "save_requests": 1, "window_id": 10}
        paths, calls = {}, []
        receiver = SimpleNamespace(pid=111, poll=lambda: None, wait=lambda timeout: None)
        open_result = {"app": {"pid": 222, "is_active": False}, "foreground_changed": False,
                       "frontmost_pid_before": 999, "frontmost_pid_after": 999}
        if opened:
            open_result.update(opened)

        def start(command, **kwargs):
            paths["state"] = Path(command[1])
            paths["directory"] = Path(command[2])
            paths["state"].write_text(json.dumps(state))
            return receiver

        editor = {"identifier": "draft-editor", "target": "editor"}
        save = {"identifier": "save-document", "target": "save"}
        panel = {"role": "AXButton", "name": "Save", "enabled": True, "target": "panel"}
        observations = [([editor], False), ([save], False)]
        if delayed_panel:
            observations.append(([], False))
        observations.append(([panel] if panel_controls is None else panel_controls, False))

        def command(args):
            calls.append(args)
            if args[1:3] == ["space", "list"]:
                return {"displays": [{"spaces": [{"id": 2, "type": 0, "is_visible": False}]}]}
            if args[1:3] == ["space", "window"]:
                return {"move_target": move_target}
            if args[1:3] == ["space", "move-window"]:
                self.assertIsNotNone(move_target, "missing target must be rejected before dispatch")
                return {"status": "confirmed", "foreground_changed": False, "visible_spaces_changed": False}
            if args[1:3] == ["app", "open"]:
                state.update(document or {})
                paths["state"].write_text(json.dumps(state))
                reader = {"activations": reader_activations, "documents": [{"content": PROBE.CONTENT}]}
                (paths["directory"] / "state-222.json").write_text(json.dumps(reader))
                return open_result
            if "panel" in args:
                if not artifact_missing:
                    (paths["directory"] / "artifact.txt").write_text(PROBE.CONTENT if content is None else content)
            return {"status": "confirmed", "foreground_changed": False}

        wait_file = PROBE.wait_file
        def wait(path, timeout=5):
            if artifact_missing and path.name == "artifact.txt":
                raise RuntimeError("expected disposable file was not created")
            wait_file(path, timeout)

        with tempfile.TemporaryDirectory() as output_directory, \
             patch.object(PROBE.subprocess, "run", return_value=SimpleNamespace(stdout=b"unlocked")), \
             patch.object(PROBE.subprocess, "Popen", side_effect=start), patch.object(PROBE, "wait_file", side_effect=wait), \
             patch.object(PROBE, "run", side_effect=command), patch.object(PROBE, "explore", side_effect=observations), \
             patch.object(PROBE, "make_reader", side_effect=lambda directory: directory / "Reader.app"), \
             patch.object(PROBE.time, "sleep"), patch.object(PROBE.os, "killpg"), patch.object(PROBE.os, "kill"):
            if through_main:
                output = Path(output_directory) / "report.json"
                with patch("sys.argv", ["probe", "--cli", "/mock/cueward", "--output", str(output)]):
                    PROBE.main()
                return json.loads(output.read_text()), calls
            return PROBE.probe(Path("/mock/cueward"), move_target is None), calls

    def test_matching_artifact_and_background_evidence_complete(self):
        report, _ = self.scenario()
        self.assertEqual(report["status"], "completed")

    def test_activation_or_foreground_change_cannot_complete(self):
        cases = [
            {"document": {"active": True}},
            {"document": {"activations": 1}},
            {"document": {"foreground_changes": 1}},
            {"reader_activations": 1},
            {"opened": {"foreground_changed": True}},
            {"opened": {"app": {"pid": 222, "is_active": True}}},
            {"opened": {"frontmost_pid_after": 888}},
        ]
        for case in cases:
            with self.subTest(case=case):
                report, _ = self.scenario(**case)
                self.assertTrue(report["file_content_matches"])
                self.assertTrue(report["cross_app_content_matches"])
                self.assertEqual(report["status"], "background_interference")

    def test_missing_reader_activation_evidence_cannot_complete(self):
        report, _ = self.scenario(reader_activations=None)
        self.assertNotEqual(report["status"], "completed")

    def test_mismatched_artifact_cannot_complete(self):
        report, _ = self.scenario(content="wrong bytes")
        self.assertEqual(report["status"], "artifact_verification_failed")

    def test_delayed_panel_is_observed_without_repeating_save(self):
        report, calls = self.scenario(delayed_panel=True)
        self.assertEqual(report["status"], "completed")
        self.assertEqual(sum("save" in args for args in calls), 1)
        self.assertEqual(sum("panel" in args for args in calls), 1)

    def test_missing_move_target_is_rejected_before_dispatch(self):
        with self.assertRaisesRegex(RuntimeError, "move target"):
            self.scenario(move_target=None)

    def test_disabled_or_ambiguous_controls_are_not_pressed(self):
        disabled = {"role": "AXButton", "name": "Save", "enabled": False, "target": "panel"}
        enabled = dict(disabled, enabled=True)
        for controls in ([disabled], [enabled, enabled], [dict(enabled, target=None)]):
            with self.subTest(controls=controls):
                report, calls = self.scenario(panel_controls=controls)
                self.assertEqual(report["status"], "save_control_unavailable")
                self.assertFalse(any("panel" in args for args in calls))
                self.assertFalse(any(args[1:3] == ["app", "open"] for args in calls))

    def test_panel_deadline_preserves_incomplete_traversal_evidence(self):
        with patch.object(PROBE, "explore", side_effect=[([], False), ([], True)]) as observe, \
             patch.object(PROBE.time, "monotonic", side_effect=[0, 0.1, 3.1]), patch.object(PROBE.time, "sleep"):
            controls, truncated = PROBE.wait_panel_save(Path("/mock/cueward"), 111)
        self.assertEqual(controls, [])
        self.assertTrue(truncated)
        self.assertEqual(observe.call_count, 2)

    def test_missing_artifact_is_reported_at_real_entry_point_without_opening_reader(self):
        report, calls = self.scenario(artifact_missing=True, through_main=True)
        self.assertEqual(report["status"], "artifact_not_created")
        self.assertFalse(report["file_created"])
        self.assertEqual(report["save_controls"], [{"enabled": True, "has_target": True}])
        self.assertIn("not created", report["error"])
        self.assertFalse(any(args[1:3] == ["app", "open"] for args in calls))


if __name__ == "__main__":
    unittest.main()
