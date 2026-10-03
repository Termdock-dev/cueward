"""Exercise receipt retention and fresh AX selection without desktop actions."""
import json
import subprocess
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from observed_cli import ObservedCLI, panel_controls, sole_target


class ObservedCLITests(unittest.TestCase):
    def test_external_result_and_actual_receipt(self):
        observer = Mock(mark=Mock(side_effect=["before", "after"]))
        result = SimpleNamespace(returncode=0, stdout='<external source="test">\n{"status":"sent_unverified","value":"&lt;/external&gt;"}\n</external>', stderr="")
        with patch("observed_cli.subprocess.run", return_value=result) as dispatch:
            commands = ObservedCLI("/owned/cueward", observer)
            self.assertEqual(commands.call("app", "press", "--target", "token")["value"], "</external>")
        dispatch.assert_called_once_with(["/owned/cueward", "app", "press", "--target", "token"],
                                         capture_output=True, text=True, timeout=45)
        self.assertEqual(commands.receipts[0]["status"], "sent_unverified")
        self.assertEqual(commands.receipts[0]["end_marker"], "after")
        self.assertGreaterEqual(commands.receipts[0]["elapsed_ms"], 0)

    def test_uncertain_outcome_never_replayed_or_erased_by_marker_failure(self):
        for error in (subprocess.TimeoutExpired("cueward", 45), OSError("launch error"), ValueError("bad JSON")):
            with self.subTest(error=error):
                observer = Mock(mark=Mock(side_effect=["before", RuntimeError("observer stopped")]))
                commands = ObservedCLI("/owned/cueward", observer)
                with patch("observed_cli.subprocess.run", side_effect=error) as dispatch:
                    with self.assertRaisesRegex(RuntimeError, "outcome unknown"):
                        commands.call("app", "press")
                self.assertEqual(dispatch.call_count, 1)
                self.assertEqual(commands.receipts[0]["status"], "unknown_outcome")
                self.assertIn("observer stopped", commands.receipts[0]["observation_error"])

    def test_tool_error_survives_observer_error(self):
        commands = ObservedCLI("/owned/cueward", Mock(mark=Mock(side_effect=["before", RuntimeError("gone")])))
        with patch("observed_cli.subprocess.run", return_value=SimpleNamespace(returncode=1, stdout="", stderr="target changed")):
            with self.assertRaisesRegex(RuntimeError, "target changed"):
                commands.call("app", "press")
        self.assertEqual(commands.receipts[0]["status"], "tool_error")

    def test_successful_dispatch_with_failed_observation_stops_caller(self):
        commands = ObservedCLI("/owned/cueward", Mock(mark=Mock(side_effect=["before", RuntimeError("gone")])))
        with patch("observed_cli.subprocess.run", return_value=SimpleNamespace(returncode=0, stdout='{"status":"confirmed"}', stderr="")):
            with self.assertRaisesRegex(RuntimeError, "do not replay"):
                commands.call("app", "set-value")
        self.assertEqual(commands.receipts[0]["status"], "confirmed")

    def test_exploration_uses_all_fresh_windows_and_retains_partial_tree(self):
        commands = ObservedCLI("/owned/cueward", None)
        commands.call = Mock(side_effect=[{"nodes": [{"ref": "w0"}, {"ref": "menu"}, {"ref": "w2"}], "truncated": False},
                                         {"nodes": [{"identifier": "one"}], "truncated": False},
                                         {"nodes": [{"identifier": "two"}], "truncated": True}])
        nodes, incomplete = commands.explore(123)
        self.assertEqual([n["identifier"] for n in nodes], ["one", "two"])
        self.assertTrue(incomplete)
        self.assertEqual(commands.call.call_args_list[-1].args,
                         ("app", "inspect", "--pid", 123, "--root", "w2", "--depth", 10, "--limit", 500))

    def test_text_without_enabled_attribute_uses_issued_target(self):
        self.assertEqual(sole_target([{"identifier": "editor", "target": "fresh"}], "editor"), "fresh")
        for nodes in ([{"identifier": "editor", "target": "fresh", "enabled": False}],
                      [{"identifier": "editor"}], [{"identifier": "editor", "target": "a"}] * 2):
            with self.assertRaises(RuntimeError):
                sole_target(nodes, "editor")

    def test_sheet_root_is_selected_without_label_or_bounds_deduplication(self):
        save = {"role": "AXButton", "name": "Save", "enabled": True, "target": "root-sheet", "ref": "w1.0.9"}
        nodes = [{"role": "AXWindow", "ref": "w0"}, {"role": "AXSheet", "ref": "w0.6"},
                 dict(save, ref="w0.6.0.9", target="nested"), {"role": "AXSheet", "ref": "w1"}, save]
        self.assertEqual(panel_controls(nodes), [save])
        nodes += [{"role": "AXSheet", "ref": "w2"}, dict(save, ref="w2.0.9")]
        self.assertEqual(len(panel_controls(nodes)), 2)


if __name__ == "__main__":
    unittest.main()
