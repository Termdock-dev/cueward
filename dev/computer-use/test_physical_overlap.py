"""Offline evidence regressions, not macOS compatibility or human acceptance."""
import copy
import unittest

from physical_overlap import (COMMAND, SHIFT, TAG, background_check, evaluate,
                              foreground_check, human_check, interruption_check,
                              paired_keys, pointer_correlation, trace_check)


def event(kind, code=0, chars="", flags=0, tag=TAG, uptime=100.02, **extra):
    return {"type": kind, "key_code": code, "characters": chars, "flags": flags,
            "tag": tag, "uptime": uptime, "pointer": [20, 20], **extra}


def execution():
    return {"start_ms": 1, "end_ms": 59, "events": [],
            "observer": {"schema": 1, "receiver_pids": [200], "system_uptime_origin": 100},
            "samples": [{"ms": ms, "frontmost_pid": 100, "pointer": [ms, 20],
                         "visible_spaces": {"1": "2", "2": "3"}, "target_active": {"200": False},
                         "session_locked": False} for ms in (0, 20, 40, 60)]}


def background():
    initial = {"events": [], "text": "", "select_alls": 0, "clicks": 0,
               "object": {"x": 70, "y": 60}, "releases": 0, "scroll_y": 0}
    final = {**initial, "text": "END", "select_alls": 1, "clicks": 1,
             "object": {"x": 150, "y": 100}, "releases": 1, "dragging": False, "scroll_y": 160}
    events = []
    for i, char in enumerate("STARTaEND"):
        flag = COMMAND if i == len("START") else 0
        events += [event(10, chars=char, flags=flag), event(11, chars=char, flags=flag)]
    final["events"] = events + [event(1), event(2), event(1), event(6), event(2), event(22)]
    return initial, final


def human():
    return {"phase": "acknowledged", "started_at": 100, "uptime": 100.1,
            "physical_modifiers": 0, "text": "aA", "events": [
                event(10, chars="a", tag=0), event(11, chars="a", tag=0),
                event(10, chars="A", flags=SHIFT, tag=0), event(11, chars="A", flags=SHIFT, tag=0),
                event(5, tag=0), event(12, tag=0)]}


class PhysicalEvidenceTests(unittest.TestCase):
    def test_moving_human_pointer_is_not_automatically_interference(self):
        original = execution()
        self.assertEqual(trace_check(original, [200], 100, "9")["status"], "passed")
        self.assertEqual(original, execution())

    def test_foreground_space_activation_lock_scope_and_gap_do_not_pass(self):
        variants = []
        for field, value in (("frontmost_pid", 101), ("visible_spaces", {"1": "9"}),
                             ("target_active", {"200": True}), ("session_locked", True), ("ms", 200)):
            data = execution(); data["samples"][-1][field] = value; variants.append(data)
        data = execution(); data["observer"]["receiver_pids"] = [201]; variants.append(data)
        data = execution(); data["events"] = [{"type": "activation", "ms": 30, "pid": 200}]; variants.append(data)
        data = execution(); data["events"] = [{"type": "activation", "ms": 30, "pid": 101}]; variants.append(data)
        data = execution(); data["events"] = [{"type": "observation_error", "ms": 30}]; variants.append(data)
        data = execution(); data["samples"] = data["samples"][1:]; variants.append(data)
        for variant in variants:
            with self.subTest(variant=variant):
                self.assertNotEqual(trace_check(variant, [200], 100, "9")["status"], "passed")

    def test_background_requires_content_effects_and_modifier_release_events(self):
        initial, final = background()
        self.assertEqual(background_check(initial, final, "START", "END")["status"], "passed")
        variants = []
        for field, value in (("text", "other"), ("object", initial["object"]), ("scroll_y", 0),
                             ("clicks", 0), ("dragging", True), ("select_alls", 0)):
            variant = copy.deepcopy(final); variant[field] = value; variants.append(variant)
        for kind, change in ((10, {"characters": "physical"}), (11, {"flags": SHIFT}), (1, {"tag": 0})):
            variant = copy.deepcopy(final)
            next(e for e in variant["events"] if e["type"] == kind).update(change)
            variants.append(variant)
        variant = copy.deepcopy(final); variant["events"] = []; variants.append(variant)
        variant = copy.deepcopy(final); variant["events"].pop(1); variants.append(variant)
        for variant in variants:
            with self.subTest(variant=variant):
                self.assertEqual(background_check(initial, variant, "START", "END")["status"], "failed")

    def test_key_pairs_allow_overlapping_physical_keys_but_not_missing_release(self):
        events = [event(10, 0), event(10, 1), event(11, 0), event(11, 1)]
        self.assertTrue(paired_keys(events))
        self.assertFalse(paired_keys(events[:-1]))
        self.assertFalse(paired_keys([event(11)]))
        self.assertFalse(paired_keys([event(10), event(10), event(11)]))
        self.assertTrue(paired_keys([event(10), event(10, is_repeat=True), event(11)]))
        self.assertFalse(paired_keys([event(10, is_repeat=True), event(11)]))

    def test_human_needs_challenge_effect_shift_movement_and_explicit_release(self):
        args = ("START", "END", 100, 100.1)
        self.assertEqual(human_check(human(), *args)["status"], "passed")
        for field, value in (("text", "START"), ("physical_modifiers", SHIFT), ("event_log_full", True)):
            state = human(); state[field] = value
            self.assertEqual(human_check(state, *args)["status"], "failed")
        state = human(); state["events"][0]["tag"] = TAG
        self.assertEqual(human_check(state, *args)["status"], "failed")
        state = human(); state["events"].pop(1)
        self.assertEqual(human_check(state, *args)["status"], "failed")
        state = human(); state["phase"] = "finished"
        self.assertEqual(human_check(state, *args)["status"], "unverified")
        state = human(); state["events"] = [e for e in state["events"] if e["type"] != 5]
        self.assertEqual(human_check(state, *args)["status"], "failed")

    def test_pointer_requires_shared_clock_and_observations_outside_human_window(self):
        data = execution()
        mouse = [event(5, tag=0, uptime=100 + s["ms"] / 1000,
                       source="global_mouse", pointer=s["pointer"]) for s in data["samples"]]
        self.assertEqual(pointer_correlation(data, mouse)["status"], "passed")
        self.assertEqual(pointer_correlation(data, [])["status"], "unverified")
        data["observer"].pop("system_uptime_origin")
        self.assertEqual(pointer_correlation(data, mouse)["status"], "unverified")
        data["observer"]["system_uptime_origin"] = float("nan")
        self.assertEqual(pointer_correlation(data, mouse)["status"], "unverified")

    def test_interruption_needs_partial_dispatch_and_actual_original_receiver_release(self):
        action = {"name": "interrupted_drag", "before": {"events": [], "releases": 0},
                  "after": {"events": [event(1), event(6), event(2)], "releases": 1,
                            "dragging": False, "interruption_triggered": True},
                  "result": {"status": "partially_sent", "events_sent": 3, "interruption": "window changed; take a new snapshot"}}
        self.assertEqual(interruption_check([action])["status"], "passed")
        variants = []
        for section, field, value in (("result", "status", "sent_unverified"), ("result", "events_sent", 52),
                                      ("after", "releases", 0), ("after", "interruption_triggered", False),
                                      ("after", "dragging", True)):
            variant = copy.deepcopy(action); variant[section][field] = value; variants.append(variant)
        variant = copy.deepcopy(action); variant["after"]["events"].pop(); variants.append(variant)
        for variant in variants:
            self.assertEqual(interruption_check([variant])["status"], "failed")
        variant = copy.deepcopy(action)
        variant["before"]["canvas_callbacks"] = {"down": 0}
        variant["after"].update(canvas_callbacks={"down": 0}, releases=0)
        check = interruption_check([variant])
        self.assertEqual(check["status"], "unverified")
        self.assertTrue(check["dispatch_stopped"] and check["original_receiver_up"])
        self.assertFalse(check["receiver_gesture_released"])
        self.assertEqual(interruption_check([])["status"], "unverified")

    def test_foreground_refusal_is_not_proven_just_by_an_error(self):
        action = {"name": "foreground_refusal", "error": "target app is in the foreground; input stopped",
                  "receipt": {"status": "tool_error", "exit_code": 1}, "before": {"active": True, "events": []},
                  "after": {"events": [event(5, tag=0)]}}
        self.assertEqual(foreground_check([action])["status"], "passed")
        action["after"]["events"].append(event(1))
        self.assertEqual(foreground_check([action])["status"], "failed")

    def test_missing_evidence_cannot_crash_or_pass(self):
        for report in ({}, {"execution": {}}, {"execution": {"observer": {}}}):
            self.assertEqual(evaluate(report)["status"], "unverified")

    def complete_report(self):
        initial, final = background()
        state = human()
        state["events"] += [event(5, tag=0, uptime=100 + s["ms"] / 1000,
                                 source="global_mouse", pointer=s["pointer"]) for s in execution()["samples"]]
        actions = [{"name": name, "receipt": {"start_marker": "start", "end_marker": "end"}}
                   for name in ("text", "shortcut", "replacement", "click", "drag", "scroll")]
        actions += [{"name": "interrupted_drag", "before": {"events": [], "releases": 0},
                     "after": {"events": [event(1), event(6), event(2)], "releases": 1,
                               "dragging": False, "interruption_triggered": True},
                     "result": {"status": "partially_sent", "events_sent": 3, "interruption": "window changed"}},
                    {"name": "foreground_refusal", "error": "target app is in foreground",
                     "receipt": {"status": "tool_error", "exit_code": 1},
                     "before": {"active": True, "events": []}, "after": {"events": []}}]
        return {"execution": execution(), "receiver_pids": [200], "human_pid": 100, "target_space": "9",
                "first_text": "START", "last_text": "END", "background_initial": initial, "background_final": final,
                "human_final": state, "actions": actions, "marker_times": {"start": 1, "end": 59, "early": 5}}

    def test_positive_subcases_cannot_hide_unrun_midflight_foreground_case(self):
        assessment = evaluate(self.complete_report())
        self.assertEqual(assessment["status"], "unverified")
        for name, check in assessment["checks"].items():
            self.assertEqual(check["status"], "unverified" if name == "midflight_foreground" else "passed", name)

    def test_overlap_uses_each_command_interval_not_the_entire_round(self):
        report = self.complete_report()
        report["actions"][0]["receipt"]["end_marker"] = "early"
        check = evaluate(report)["checks"]["command_overlap"]
        self.assertEqual(check["status"], "unverified")
        self.assertEqual(check["actions"]["text"], {"key_downs": 0, "mouse_moves": 0})

    def test_failed_receiver_effect_dominates_unverified_subcases(self):
        report = self.complete_report(); report["background_final"]["object"]["x"] = 70
        self.assertEqual(evaluate(report)["status"], "failed")


if __name__ == "__main__":
    unittest.main()
