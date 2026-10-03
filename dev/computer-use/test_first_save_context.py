"""Context comparison safety and lifecycle tests; not desktop acceptance."""
import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

SPEC = importlib.util.spec_from_file_location("first_save_context", Path(__file__).with_name("first-save-context-probe.py"))
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class ContextTests(unittest.TestCase):
    def scenario(self, context="visible", enabled=True, ambiguous=False, content=None, membership=None):
        commands, calls = Mock(), []
        panel = {"role": "AXButton", "name": "Save", "enabled": enabled, "target": "initial-panel", "ref": "w1.0"}
        current = dict(panel, target="after-move-panel")
        commands.explore.side_effect = [([{"identifier": "draft-editor", "target": "editor"}], False),
                                        ([{"identifier": "save-document", "enabled": True, "target": "save-command"}], False)]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "receiver.json").write_text(json.dumps({"panel_window_id": 20}))
            def dispatch(*arguments):
                calls.append(arguments)
                if arguments[:2] == ("space", "list"):
                    return {"displays": [{"spaces": [{"id": 3, "type": 0, "is_visible": False}]}]}
                if arguments[:2] == ("space", "window"):
                    return {"move_target": "fresh-move", "space_ids": [3] if membership is None else membership}
                if "initial-panel" in arguments or "after-move-panel" in arguments:
                    if content is not False:
                        (directory / "artifact.txt").write_text(PROBE.BASELINE.CONTENT if content is None else content)
                return {"status": "confirmed", "foreground_changed": False, "visible_spaces_changed": False}
            commands.call.side_effect = dispatch
            observed = [([panel] * (2 if ambiguous else 1), False, [panel])]
            if context == "warm":
                observed.append(([current], True, [current]))
            report = {"panel_save_dispatch_count": 0}
            with patch.object(PROBE, "observe_panel", side_effect=observed), \
                 patch.object(PROBE.time, "monotonic", side_effect=[0, 0, 6]), patch.object(PROBE.time, "sleep"):
                PROBE.exercise(commands, 100, 10, directory, context, report)
        return report, calls

    def test_verified_bytes_require_one_dispatch(self):
        report, calls = self.scenario()
        self.assertEqual(report["status"], "artifact_verified")
        self.assertEqual(report["panel_save_dispatch_count"], 1)
        self.assertEqual(sum("initial-panel" in args for args in calls), 1)

    def test_warm_panel_is_reobserved_after_move(self):
        report, calls = self.scenario(context="warm")
        self.assertTrue(report["panel_traversal_incomplete"])
        self.assertEqual(report["status"], "artifact_verified")
        self.assertFalse(any("initial-panel" in args for args in calls))
        self.assertEqual(sum("after-move-panel" in args for args in calls), 1)

    def test_disabled_ambiguous_or_missing_enable_cannot_dispatch(self):
        for arguments in ({"enabled": False}, {"enabled": None}, {"ambiguous": True}):
            report, calls = self.scenario(**arguments)
            self.assertEqual(report["panel_save_dispatch_count"], 0)
            self.assertFalse(any("initial-panel" in args for args in calls))

    def test_missing_or_wrong_artifact_never_replayed(self):
        for content in (False, "wrong"):
            report, calls = self.scenario(content=content)
            self.assertEqual(report["status"], "artifact_not_created_or_mismatched")
            self.assertEqual(sum("initial-panel" in args for args in calls), 1)

    def test_unconfirmed_panel_membership_blocks_save(self):
        with self.assertRaisesRegex(RuntimeError, "membership"):
            self.scenario(context="warm", membership=[1])

    def test_existing_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "existing.json"
            path.write_text("previous evidence")
            with self.assertRaises(FileExistsError):
                PROBE.probe("/mock/cueward", "inactive", path)
            self.assertEqual(path.read_text(), "previous evidence")

    def test_receiver_stops_before_directory_cleanup_and_error_is_preserved(self):
        observed = []
        receiver = Mock(pid=100, poll=Mock(return_value=None))
        def launch(directory):
            receiver.terminate.side_effect = lambda: observed.append(directory.exists())
            return receiver
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(PROBE, "compile_observer", return_value=Path("/mock/observer")), \
             patch.object(PROBE, "launch_fixture", side_effect=launch), \
             patch.object(PROBE, "observe_exercise", side_effect=RuntimeError("primary error")):
            output = Path(temporary) / "report.json"
            report = PROBE.probe("/mock/cueward", "inactive", output)
            self.assertEqual(json.loads(output.read_text())["error"], "primary error")
        self.assertEqual(report["status"], "diagnostic_error")
        self.assertEqual(observed, [True])
        receiver.terminate.assert_called_once()
        receiver.wait.assert_called_once_with(timeout=5)


if __name__ == "__main__":
    unittest.main()
