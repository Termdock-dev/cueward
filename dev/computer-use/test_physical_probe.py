"""Bounded diagnostic controls without desktop operations or keyboard monitoring."""
import importlib.util
import json
from pathlib import Path
import tempfile
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch

SPEC = importlib.util.spec_from_file_location("physical_probe", Path(__file__).with_name("physical-overlap-probe.py"))
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class PreparationTests(unittest.TestCase):
    def move(self, changed=None, membership=None):
        moved = {"status": "confirmed", "visible_spaces_changed": False, "window_changed": False,
                 "foreground_changed": True, **(changed or {})}
        commands = Mock()
        commands.call.side_effect = [
            {"displays": [{"spaces": [{"id": 9, "type": 0, "is_visible": False}]}]},
            {"move_target": "fresh-owned-token"}, moved,
            {"space_ids": [9] if membership is None else membership}]
        report = {}
        result = PROBE.owned_move(commands, 200, report)
        return result, report, commands

    def test_preparation_records_app_switch_without_importing_it_into_execution(self):
        destination, report, commands = self.move()
        self.assertEqual(destination, "9")
        self.assertTrue(report["setup_moves"][0]["foreground_changed"])
        self.assertEqual(commands.call.call_args_list[-1].args, ("space", "window", "--id", 200))

    def test_changed_window_visible_space_or_wrong_membership_aborts(self):
        for changed, membership in (({"status": "unconfirmed"}, None), ({"visible_spaces_changed": True}, None),
                                    ({"window_changed": True}, None), ({}, [8]), ({}, [9, 10])):
            with self.subTest(changed=changed, membership=membership), self.assertRaises(RuntimeError):
                self.move(changed, membership)

    def test_no_inactive_space_does_not_create_or_move_a_space(self):
        commands = Mock(); commands.call.return_value = {"displays": []}
        with self.assertRaises(RuntimeError):
            PROBE.owned_move(commands, 200, {})
        commands.call.assert_called_once_with("space", "list")


class DispatchTests(unittest.TestCase):
    def test_uncertain_action_is_recorded_once_and_not_replayed(self):
        with tempfile.TemporaryDirectory() as temporary:
            state = Path(temporary) / "state.json"
            state.write_text(json.dumps({"pid": 200, "window_id": 300, "text": "unchanged"}))
            report = {"actions": []}
            runner = PROBE.Round("unused-cli", Mock(), report, {"background": state})
            runner.snapshot = Mock(return_value={"input_target": "fresh-token"})
            runner.commands = Mock()
            runner.commands.receipts = [{"status": "unknown_outcome"}]
            runner.commands.call.side_effect = RuntimeError("unknown outcome; inspect before retrying")
            with patch.object(PROBE.time, "sleep"):
                runner.action("text", "background", lambda shot, value: ["type-text", "--target", shot["input_target"], "--text", "synthetic"])
            runner.commands.call.assert_called_once()
            self.assertEqual(len(report["actions"]), 1)
            action = report["actions"][0]
            self.assertNotIn("result", action)
            self.assertIn("unknown outcome", action["error"])
            self.assertEqual(action["receipt"]["status"], "unknown_outcome")
            self.assertEqual(action["before"], action["after"])

    def test_snapshot_owner_mismatch_never_claims_an_image_for_cleanup(self):
        with tempfile.TemporaryDirectory() as temporary:
            state = Path(temporary) / "state.json"
            state.write_text(json.dumps({"pid": 200, "window_id": 300}))
            runner = PROBE.Round("unused-cli", Mock(), {"actions": []}, {"background": state})
            runner.commands = Mock()
            runner.commands.call.return_value = {"window": {"owner_pid": 201}, "screenshot": {"path": "/unowned-image"}}
            with self.assertRaises(RuntimeError):
                runner.snapshot("background")
            self.assertEqual(runner.images, set())

    def test_round_deadline_does_not_dispatch_remaining_input(self):
        runner = PROBE.Round("unused-cli", Mock(), {"actions": []}, {})
        runner.action = Mock()
        with patch.object(PROBE.time, "monotonic", side_effect=[0, 29]), self.assertRaises(RuntimeError):
            runner.main_actions()
        runner.action.assert_not_called()

    def test_expiry_during_snapshot_is_checked_before_dispatch(self):
        with tempfile.TemporaryDirectory() as temporary:
            state = Path(temporary) / "state.json"
            state.write_text(json.dumps({"pid": 200, "window_id": 300}))
            report = {"actions": []}
            runner = PROBE.Round("unused-cli", Mock(), report, {"background": state})
            runner.input_deadline = 1
            runner.snapshot = Mock(return_value={"input_target": "fresh-token"})
            runner.commands = Mock(); runner.commands.receipts = [{"status": "observed"}]
            with patch.object(PROBE.time, "monotonic", return_value=2), patch.object(PROBE.time, "sleep"):
                runner.action("text", "background", lambda *args: ["type-text"])
            runner.commands.call.assert_not_called()
            self.assertTrue(report["actions"][0]["pre_dispatch_refusal"])


class CleanupTests(unittest.TestCase):
    def test_corrupt_state_does_not_prevent_owned_process_or_image_cleanup(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, image, preserved = root / "state.json", root / "owned.png", root / "preserved.txt"
            state.write_text("corrupt evidence"); image.write_bytes(b"synthetic image"); preserved.write_text("preserve")
            process = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
            report = {}
            try:
                PROBE.cleanup([process], {"background": state}, Mock(images={image}), report)
                self.assertIsNotNone(process.poll())
                self.assertTrue(report["cleanup"]["owned_processes_stopped"])
                self.assertTrue(report["cleanup"]["errors"])
                self.assertFalse(image.exists())
                self.assertEqual(state.read_text(), "corrupt evidence")
                self.assertEqual(preserved.read_text(), "preserve")
            finally:
                if process.poll() is None:
                    process.kill(); process.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
