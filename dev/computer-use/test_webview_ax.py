"""Offline selection/effect/one-dispatch checks, not WebView compatibility."""

import copy
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

from webview_ax import BUTTON_NAME, click_effect, select_button, wait_click

SPEC = importlib.util.spec_from_file_location("webview_ax_probe", Path(__file__).with_name("webview-ax-probe.py"))
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


def state():
    return {"pid": 321, "window_id": 42, "content_top": 22, "active": False, "activations": 0,
            "native_events": [], "dom": {"clicks": 0, "events": [],
                                           "regions": {"button": {"x": 20, "y": 20, "width": 160, "height": 40}}}}


def tree():
    return {"window": {"owner_pid": 321, "window_id": 42, "bounds": {"x": 100, "y": 200}},
            "accessibility": {"truncated": False, "nodes": [
                {"ref": "0.0", "role": "AXWebArea", "name": ""},
                {"ref": "0.0.0", "role": "AXButton", "name": BUTTON_NAME, "enabled": True,
                 "actions": ["AXPress"], "target": "fresh-page-target",
                 "bounds": {"x": 120, "y": 242, "width": 160, "height": 40}},
                {"ref": "0.1", "role": "AXButton", "name": "", "actions": ["AXPress"]}]}}


def clicked(before):
    after = copy.deepcopy(before)
    after["dom"]["clicks"] += 1
    after["dom"]["events"].append({"type": "click", "trusted": True})
    return after


class SelectionTests(unittest.TestCase):
    def test_selects_page_button_not_unnamed_chrome(self):
        self.assertEqual(select_button(tree(), state())["target"], "fresh-page-target")

    def test_missing_and_ambiguous_buttons_do_not_dispatch(self):
        for nodes in ([], tree()["accessibility"]["nodes"] * 2):
            value = tree(); value["accessibility"]["nodes"] = nodes
            with self.assertRaises(RuntimeError):
                select_button(value, state())

    def test_partial_identity_disabled_unpressable_and_geometry_mismatch_are_rejected(self):
        variants = []
        value = tree(); value["accessibility"]["truncated"] = True; variants.append(value)
        value = tree(); value["window"]["owner_pid"] = 999; variants.append(value)
        value = tree(); value["window"]["window_id"] = 43; variants.append(value)
        for field, replacement in (("enabled", False), ("enabled", None), ("actions", []), ("target", ""),
                                   ("bounds", {}), ("bounds", {"x": float("nan"), "y": 242, "width": 160, "height": 40}),
                                   ("ref", "0.1")):
            value = tree(); value["accessibility"]["nodes"][1][field] = replacement; variants.append(value)
        for value in variants:
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                select_button(value, state())


class EffectTests(unittest.TestCase):
    def test_ax_click_does_not_require_dom_mouseup_and_keeps_first_separate(self):
        before = state(); after = clicked(before)
        self.assertEqual(click_effect(before, after)["status"], "passed")
        second = clicked(after)
        self.assertEqual(click_effect(after, second)["status"], "passed")
        self.assertEqual(click_effect(before, second)["status"], "failed")

    def test_receipt_cannot_substitute_for_single_trusted_effect_or_isolation(self):
        before = state()
        variants = [state()]
        for field, value in (("active", True), ("activations", 1), ("pid", 999), ("window_id", 43),
                             ("native_events", [{}])):
            after = clicked(before); after[field] = value; variants.append(after)
        after = clicked(before); after["dom"]["events"] = []; variants.append(after)
        after = clicked(before); after["dom"]["events"][0]["trusted"] = False; variants.append(after)
        for after in variants:
            with self.subTest(after=after):
                self.assertEqual(click_effect(before, after)["status"], "failed")

    def test_event_log_replacement_cannot_be_counted_as_success(self):
        before = clicked(state()); after = clicked(before)
        after["dom"]["events"][0]["trusted"] = False
        self.assertEqual(click_effect(before, after)["status"], "failed")

    def test_waits_for_new_click_without_input_or_prior_click_reuse(self):
        before = clicked(state()); after = clicked(before)
        read = Mock(side_effect=[before, after])
        with patch("webview_ax.time.sleep"):
            observed, settling = wait_click(read, before)
        self.assertEqual(observed, after)
        self.assertEqual(settling["status"], "new_click_observed")
        self.assertEqual(read.call_count, 2)
        _, settling = wait_click(lambda: before, before, timeout=0)
        self.assertEqual(settling["status"], "no_click_within_budget")


class DispatchTests(unittest.TestCase):
    def test_uncertain_press_is_recorded_once_and_never_replayed(self):
        before = state(); report = {"actions": []}
        commands = Mock(receipts=[{"status": "unknown_outcome"}])
        commands.call.side_effect = [tree(), RuntimeError("unknown; do not replay")]
        with patch.object(PROBE.WEB.OWNED, "read_state", return_value=before), \
             patch.object(PROBE, "wait_click", return_value=(before, {"status": "no_click_within_budget"})):
            action = PROBE.press_once(commands, Path("unused"), report, "first_ax_press", (321, 42))
        self.assertEqual(commands.call.call_count, 2)
        self.assertEqual(commands.call.call_args.args[:2], ("window", "press"))
        self.assertIn("unknown", action["error"])
        self.assertEqual(action["effect"]["status"], "failed")
        self.assertEqual(report["actions"], [action])

    def test_foreground_transition_before_press_stops_without_dispatch(self):
        before = state(); active = dict(before, active=True)
        commands = Mock(); commands.call.return_value = tree()
        with patch.object(PROBE.WEB.OWNED, "read_state", side_effect=[before, active]):
            with self.assertRaises(RuntimeError):
                PROBE.press_once(commands, Path("unused"), {"actions": []}, "first_ax_press", (321, 42))
        self.assertEqual(commands.call.call_count, 1)

    def test_changed_owned_identity_is_rejected_before_inspection_or_press(self):
        for field, value in (("pid", 999), ("window_id", 43)):
            foreign = dict(state(), **{field: value})
            for states, expected_calls in (([foreign], 0), ([state(), foreign], 1)):
                commands = Mock(); commands.call.return_value = tree()
                with patch.object(PROBE.WEB.OWNED, "read_state", side_effect=states):
                    with self.assertRaises(RuntimeError):
                        PROBE.press_once(commands, Path("unused"), {"actions": []}, "first_ax_press", (321, 42))
                self.assertEqual(commands.call.call_count, expected_calls)


if __name__ == "__main__":
    unittest.main()
