"""Offline DOM effect/settling checks, not native WebView compatibility."""
import copy
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("webview_context", Path(__file__).with_name("webview-context-probe.py"))
PROBE = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(PROBE)


def state():
    return {"dom": {"clicks": 0, "releases": 0, "dragging": False,
                    "object": {"x": 100, "y": 80}, "events": []}}


def event(kind, buttons):
    return {"type": kind, "buttons": buttons}


class EffectsTests(unittest.TestCase):
    def test_click_needs_exact_counter_effect_and_single_dom_click(self):
        before = state(); after = state()
        after["dom"].update(clicks=1, events=[event("mousedown", 0), event("mouseup", 0), event("click", 0)])
        self.assertEqual(PROBE.effect_check(before, after, "first_single_click")["status"], "passed")
        after["dom"]["clicks"] = 2
        self.assertEqual(PROBE.effect_check(before, after, "first_single_click")["status"], "failed")
        after["dom"].update(clicks=1, events=[])
        self.assertEqual(PROBE.effect_check(before, after, "first_single_click")["status"], "failed")

    def test_subsequent_single_click_is_not_aggregated_into_first_click_success(self):
        before = state(); before["dom"].update(clicks=1, events=[event("click", 0)])
        after = copy.deepcopy(before); after["dom"]["clicks"] = 2; after["dom"]["events"].append(event("click", 0))
        self.assertEqual(PROBE.effect_check(before, after, "subsequent_single_click")["click_increment"], 1)
        self.assertEqual(PROBE.effect_check(state(), after, "first_single_click")["status"], "failed")

    def test_drag_needs_displacement_button_state_and_complete_release(self):
        before, after = state(), state()
        after["dom"].update(object={"x": 180, "y": 120}, releases=1,
                            events=[event("mousedown", 1), event("mousemove", 1), event("mouseup", 0)])
        self.assertEqual(PROBE.effect_check(before, after, "canvas_drag")["status"], "passed")
        for field, value in (("object", {"x": 100, "y": 80}), ("releases", 0), ("dragging", True)):
            variant = copy.deepcopy(after); variant["dom"][field] = value
            self.assertEqual(PROBE.effect_check(before, variant, "canvas_drag")["status"], "failed")
        variant = copy.deepcopy(after); variant["dom"]["events"][1]["buttons"] = 0
        self.assertEqual(PROBE.effect_check(before, variant, "canvas_drag")["status"], "failed")
        variant = copy.deepcopy(after); variant["dom"]["events"].pop()
        self.assertEqual(PROBE.effect_check(before, variant, "canvas_drag")["status"], "failed")


class SettlingTests(unittest.TestCase):
    def test_delayed_terminal_state_is_observed_without_dispatch(self):
        before, partial, completed = state(), state(), state()
        partial["dom"]["events"] = [event("mousedown", 0), event("mousemove", 0)]
        completed["dom"].update(events=partial["dom"]["events"] + [event("mouseup", 0)], releases=1)
        with patch.object(PROBE.OWNED, "read_state", side_effect=[partial, completed]) as read, patch.object(PROBE.time, "sleep"):
            after, settling = PROBE.wait_terminal(Path("unused"), before, "canvas_drag")
        self.assertEqual(read.call_count, 2)
        self.assertEqual(after, completed)
        self.assertEqual(settling["status"], "terminal_dom_observed")

    def test_bounded_absence_is_not_a_permanent_event_loss_claim(self):
        with patch.object(PROBE.OWNED, "read_state", return_value=state()):
            _, settling = PROBE.wait_terminal(Path("unused"), state(), "canvas_drag", timeout=0)
        self.assertEqual(settling["status"], "no_terminal_dom_within_budget")

    def test_prior_mouseup_cannot_satisfy_the_next_trial(self):
        before = state(); before["dom"]["events"] = [event("mouseup", 0), event("click", 0)]
        with patch.object(PROBE.OWNED, "read_state", return_value=before):
            _, settling = PROBE.wait_terminal(Path("unused"), before, "subsequent_single_click", timeout=0)
        self.assertEqual(settling["status"], "no_terminal_dom_within_budget")


if __name__ == "__main__":
    unittest.main()
