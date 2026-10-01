"""Check saved trial evidence and receiver isolation without a GUI session."""
import importlib.util
import json
from pathlib import Path
import tempfile
import subprocess
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("webview_probe", Path(__file__).with_name("webview-probe.py"))
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class WebViewTests(unittest.TestCase):
    def scenario(self, activate_first=False, fail_first=False, fail_second_snapshot=False,
                 interrupt_first=False, fail_compile=False):
        receivers, actions, checkpoints, cleaned = [], [], [], []
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "report.json"

            def start(command, **kwargs):
                index = len(receivers)
                state = {"window_id": 10 + index, "active": False, "activations": 0,
                         "foreground_changes": 0, "pointer_different_samples": 0,
                         "observations": 1, "content_top": 0,
                         "dom": {"regions": {"button": {"x": 0, "y": 0, "width": 20, "height": 20},
                                             "canvas": {"x": 0, "y": 30}},
                                 "object": {"x": 50, "y": 50}, "events": [], "clicks": 0, "releases": 0}}
                path = Path(command[1])
                path.write_text(json.dumps(state))
                receivers.append((path, state))
                return SimpleNamespace(pid=100 + index, poll=lambda: None, wait=lambda timeout: None)

            def command(args):
                path, state = receivers[-1]
                if args[1:3] == ["window", "snapshot"]:
                    if fail_second_snapshot and len(actions) == 1 and len(receivers) == 1:
                        raise RuntimeError("snapshot observation failed")
                    return {"input_target": str(state["window_id"]), "image": {"scale_x": 1, "scale_y": 1},
                            "screenshot": {"path": str(Path(temporary) / "unused.png")}}
                action = args[2]
                if actions:
                    checkpoints.append(json.loads(output.read_text()) if output.exists() else None)
                actions.append((action, state["window_id"]))
                if state["active"]:
                    raise RuntimeError("target app is in the foreground")
                if action == "click" and len(actions) == 1:
                    if fail_first:
                        raise RuntimeError("uncertain dispatch; inspect before retrying")
                    if activate_first:
                        state.update(active=True, activations=1, foreground_changes=1)
                if action == "click":
                    state["dom"]["clicks"] += 1
                else:
                    state["dom"]["object"]["x"] += 80
                    state["dom"]["object"]["y"] += 40
                    state["dom"]["releases"] += 1
                state["dom"]["events"].append({"type": action})
                state["observations"] += 1
                path.write_text(json.dumps(state))
                if interrupt_first and len(actions) == 1:
                    return {"status": "partially_sent", "events_sent": 2, "interruption": "foreground changed"}
                return {"status": "sent_unverified", "events_sent": 2}

            def compile_or_session(args, **kwargs):
                if fail_compile and args[0] == "swiftc":
                    raise subprocess.CalledProcessError(1, args)
                return SimpleNamespace(stdout=b"unlocked")

            with patch("sys.argv", ["probe", "--cli", "/mock/cueward", "--output", str(output)]), \
                 patch.object(PROBE.subprocess, "run", side_effect=compile_or_session), \
                 patch.object(PROBE.subprocess, "Popen", side_effect=start), \
                 patch.object(PROBE, "run", side_effect=command), patch.object(PROBE.time, "sleep"), \
                 patch.object(PROBE.os, "killpg", side_effect=lambda pid, sig: cleaned.append(pid)):
                exit_code = PROBE.main()
            self.assertEqual(exit_code, 1 if (activate_first or fail_first or fail_second_snapshot or interrupt_first or fail_compile) else 0)
            return json.loads(output.read_text()), actions, checkpoints, cleaned

    def test_first_click_activation_is_saved_and_drag_still_runs(self):
        report, actions, checkpoints, cleaned = self.scenario(activate_first=True)
        first, subsequent, drag = report["trials"]
        self.assertTrue(first["target_active"])
        self.assertEqual(first["click_effects"], 1)
        self.assertEqual(subsequent["status"], "skipped_foreground")
        self.assertEqual([action for action, _ in actions], ["click", "drag"])
        self.assertNotEqual(actions[0][1], actions[-1][1])
        self.assertEqual(drag["object_displacement"], {"x": 80, "y": 40})
        self.assertEqual(drag["release_effects"], 1)
        self.assertEqual(checkpoints[0]["trials"][0]["click_effects"], 1)
        self.assertEqual(cleaned, [100, 101])

    def test_each_successful_trial_is_saved_before_next_action(self):
        report, actions, checkpoints, _ = self.scenario()
        self.assertEqual([action for action, _ in actions], ["click", "click", "drag"])
        self.assertEqual(actions[0][1], actions[1][1])
        self.assertNotEqual(actions[1][1], actions[2][1])
        self.assertEqual([len(item["trials"]) for item in checkpoints], [1, 2])
        self.assertEqual(report["trials"][-1]["release_effects"], 1)

    def test_dispatch_failure_is_saved_without_replay_and_drag_still_runs(self):
        report, actions, _, cleaned = self.scenario(fail_first=True)
        self.assertEqual(report["trials"][0]["status"], "dispatch_failed")
        self.assertIn("uncertain dispatch", report["trials"][0]["error"])
        self.assertEqual(report["trials"][1]["status"], "skipped_previous_trial_error")
        self.assertEqual([action for action, _ in actions], ["click", "drag"])
        self.assertEqual(report["trials"][-1]["release_effects"], 1)
        self.assertEqual(cleaned, [100, 101])

    def test_later_snapshot_failure_keeps_first_click_and_independent_drag(self):
        report, actions, _, _ = self.scenario(fail_second_snapshot=True)
        self.assertEqual(report["trials"][0]["click_effects"], 1)
        self.assertEqual(report["trials"][1]["status"], "observation_failed")
        self.assertEqual(report["trials"][-1]["release_effects"], 1)
        self.assertEqual([action for action, _ in actions], ["click", "drag"])

    def test_interrupted_dispatch_does_not_trigger_second_click(self):
        report, actions, _, _ = self.scenario(interrupt_first=True)
        self.assertEqual(report["trials"][0]["status"], "dispatch_interrupted")
        self.assertEqual(report["trials"][0]["posted_events"], 2)
        self.assertEqual(report["trials"][1]["status"], "skipped_previous_trial_error")
        self.assertEqual([action for action, _ in actions], ["click", "drag"])

    def test_compilation_failure_leaves_report_and_starts_no_receiver(self):
        report, actions, _, cleaned = self.scenario(fail_compile=True)
        self.assertEqual(report["status"], "recorded_with_errors")
        self.assertIn("fatal_error", report)
        self.assertEqual(actions, [])
        self.assertEqual(cleaned, [])


if __name__ == "__main__":
    unittest.main()
