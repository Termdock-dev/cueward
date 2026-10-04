"""Offline lock-evidence regressions, not macOS or locked-session acceptance."""

import copy
import json
import unittest

from lock_resources_report import PHASES, ROUTES, summarize


CLEAN = {"status": "passed", "errors": []}


def sample(phase="before_lock", index=0):
    begin, sequence = 100.0 + index * 2, 2 * index + 1
    before = {"sequence": sequence, "uptime": begin, "pid": 401, "window_id": 901}
    after = {**before, "sequence": sequence + 1, "uptime": begin + 1}
    ax = {"status": "decoded", "sequence": sequence + 1,
          "api_code": 0, "uptime": begin + 0.5}
    capture = {"status": "decoded", "sequence": sequence + 1,
               "frameCount": 1, "uptime": begin + 0.5}
    return {"phase": phase, "locked": phase == "locked", "locked_end": phase == "locked",
            "begin_uptime": begin, "end_uptime": begin + 1,
            "receiver_before": before, "receiver_after": after,
            "observations": {r: copy.deepcopy(ax if r.endswith("ax") else capture)
                             for r in ROUTES}}


def trial():
    return [sample(phase, i * 2 + j) for i, phase in enumerate(PHASES) for j in range(2)]


def route(report, phase="before_lock", name="retained_ax"):
    return report["phases"][phase]["routes"][name]


class LockResourcesReportTests(unittest.TestCase):
    def test_baseline_requires_distinct_receiver_correlated_values(self):
        data = [sample(index=0), sample(index=1)]
        report = summarize(data, cleanup=CLEAN)
        self.assertEqual(report["baseline_status"], "passed")
        self.assertEqual(report["recording_status"], "complete")
        self.assertEqual(report["lock_status"], "unverified")
        for name in ROUTES:
            result = route(report, name=name)
            self.assertEqual(result["status"], "fresh")
            self.assertEqual(result["matching_count"], 2)
            self.assertEqual(result["distinct_sequence_count"], 2)
            self.assertEqual(result["outcome_counts"], {"matching": 2})
        self.assertEqual(report["phases"]["locked"]["status"], "unverified")

    def test_lock_trial_records_all_phases_without_claiming_product_support(self):
        report = summarize(trial(), mode="lock", cleanup=CLEAN)
        self.assertEqual(report["baseline_status"], "passed")
        self.assertEqual(report["lock_status"], "passed")
        self.assertEqual(report["recording_status"], "complete")
        self.assertEqual(report["product_operation_support"], "unverified")
        self.assertTrue(all(p["receiver_status"] == "fresh" for p in report["phases"].values()))

    def test_single_matching_sample_does_not_prove_freshness(self):
        report = summarize([sample()], cleanup=CLEAN)
        self.assertEqual(report["baseline_status"], "unverified")
        self.assertEqual(route(report)["status"], "unverified")
        self.assertEqual(route(report)["matching_count"], 1)

    def test_same_matching_value_and_frozen_oracle_are_unverified(self):
        data = [sample(index=i) for i in range(2)]
        for item in data:
            for oracle in ("receiver_before", "receiver_after"):
                item[oracle]["sequence"] = 7
            for value in item["observations"].values():
                value["sequence"] = 7
        report = summarize(data, cleanup=CLEAN)
        self.assertEqual(route(report)["matching_count"], 2)
        self.assertEqual(route(report)["distinct_sequence_count"], 1)
        self.assertEqual(route(report)["status"], "unverified")
        self.assertEqual(report["phases"]["before_lock"]["receiver_status"], "unverified")
        self.assertEqual(report["baseline_status"], "unverified")

    def test_stale_requires_independent_receiver_advancement(self):
        data = [sample(index=i + 1) for i in range(2)]
        for item in data:
            for value in item["observations"].values():
                value.update(sequence=1, uptime=99.0)
        report = summarize(data, cleanup=CLEAN)
        self.assertEqual(route(report)["status"], "stale")
        self.assertEqual(route(report)["outcome_counts"], {"stale": 2})
        self.assertEqual(report["baseline_status"], "failed")
        for item in data:
            item["receiver_before"]["sequence"] = 8
            item["receiver_after"]["sequence"] = 8
        report = summarize(data, cleanup=CLEAN)
        self.assertEqual(route(report)["status"], "unverified")
        self.assertEqual(route(report)["outcome_counts"], {"unverified": 2})

    def test_api_success_without_decoded_data_is_not_fresh(self):
        data = [sample(index=i) for i in range(2)]
        for item in data:
            item["observations"]["new_screenshot"].update(status="no_frame", sequence=None,
                                                         frameCount=0, api_code=0)
        report = summarize(data, cleanup=CLEAN)
        result = route(report, name="new_screenshot")
        self.assertEqual(result["status"], "unavailable")
        self.assertEqual(result["matching_count"], 0)
        self.assertEqual(result["outcome_counts"], {"no_frame": 2})
        self.assertEqual(report["baseline_status"], "failed")

    def test_true_is_not_a_sequence_pid_timestamp_api_code_or_frame_count(self):
        variants = []
        for oracle in ("receiver_before", "receiver_after"):
            for field in ("sequence", "pid", "window_id", "uptime"):
                data = [sample(index=i) for i in range(2)]
                data[0][oracle][field] = True
                variants.append(data)
        for field in ("begin_uptime", "end_uptime"):
            data = [sample(index=i) for i in range(2)]
            data[0][field] = True
            variants.append(data)
        for name, field in (("retained_ax", "sequence"), ("retained_ax", "uptime"),
                            ("retained_ax", "api_code"), ("new_screenshot", "frameCount")):
            data = [sample(index=i) for i in range(2)]
            data[0]["observations"][name][field] = True
            variants.append(data)
        for data in variants:
            with self.subTest(data=data):
                self.assertNotEqual(summarize(data, cleanup=CLEAN)["baseline_status"], "passed")

    def test_future_sequence_and_timestamp_cannot_be_matching(self):
        for field, value in (("sequence", 99), ("uptime", 1000.0), ("uptime", 99.0)):
            data = [sample(index=i) for i in range(2)]
            for item in data:
                item["observations"]["retained_ax"][field] = value
            report = summarize(data, cleanup=CLEAN)
            self.assertEqual(route(report)["status"], "unverified")
            self.assertEqual(route(report)["matching_count"], 0)
            self.assertNotEqual(report["baseline_status"], "passed")

    def test_mixed_matching_error_is_not_fresh(self):
        data = [sample(index=i) for i in range(3)]
        data[-1]["observations"]["retained_ax"].update(status="error", sequence=None,
                                                    api_code=-25200, error="opaque API failure")
        report = summarize(data, cleanup=CLEAN)
        result = route(report)
        self.assertEqual(result["status"], "mixed")
        self.assertEqual(result["matching_count"], 2)
        self.assertEqual(result["outcome_counts"], {"error": 1, "matching": 2})
        self.assertEqual(report["baseline_status"], "failed")

    def test_counts_capture_failures_and_does_not_guess_permission_from_words(self):
        statuses = ("blank", "undecodable", "stopped", "error", "timeout", "no_frame")
        data = [sample(index=i) for i in range(len(statuses))]
        for item, status in zip(data, statuses):
            item["observations"]["retained_stream"].update(
                status=status, sequence=None, frameCount=0, error="permission denied")
        report = summarize(data, cleanup=CLEAN)
        result = route(report, name="retained_stream")
        self.assertEqual(result["status"], "mixed")
        self.assertEqual(result["outcome_counts"], dict.fromkeys(statuses, 1))
        self.assertNotIn("permission_denied", result["outcome_counts"])

    def test_permission_requires_known_ax_code_or_explicit_classification(self):
        data = [sample(index=i) for i in range(2)]
        for item in data:
            item["observations"]["retained_ax"].update(status="error", sequence=None,
                                                    api_code=-25211)
            item["observations"]["new_screenshot"].update(status="error", sequence=None,
                                                        error_kind="permission_denied")
        report = summarize(data, cleanup=CLEAN)
        for name in ("retained_ax", "new_screenshot"):
            self.assertEqual(route(report, name=name)["status"], "permission_denied")
            self.assertEqual(route(report, name=name)["outcome_counts"], {"permission_denied": 2})

    def test_phase_transition_or_non_boolean_lock_state_is_unverified(self):
        for field, value in (("locked_end", False), ("locked", 1), ("locked_end", 1)):
            data = trial()
            data[2][field] = value
            report = summarize(data, mode="lock", cleanup=CLEAN)
            self.assertEqual(report["lock_status"], "unverified")
            self.assertEqual(report["recording_status"], "unverified")
            self.assertNotEqual(route(report, "locked")["status"], "fresh")

    def test_missing_locked_and_after_phases_are_retained_not_skipped(self):
        report = summarize([sample(index=i) for i in range(2)], mode="lock", cleanup=CLEAN)
        self.assertEqual(report["baseline_status"], "passed")
        self.assertEqual(report["lock_status"], "unverified")
        self.assertEqual(report["recording_status"], "unverified")
        for phase in PHASES[1:]:
            self.assertEqual(report["phases"][phase]["sample_count"], 0)
            self.assertEqual(route(report, phase)["status"], "unverified")

    def test_all_recorded_errors_complete_research_but_cannot_pass_freshness(self):
        data = trial()
        for item in data[2:4]:
            for name, value in item["observations"].items():
                value.update(status="error", sequence=None, error="opaque observation failure")
                if name.endswith("ax"):
                    value["api_code"] = -25200
        report = summarize(data, mode="lock", cleanup=CLEAN)
        self.assertEqual(report["recording_status"], "complete")
        self.assertEqual(report["lock_status"], "failed")
        self.assertTrue(all(route(report, "locked", name)["status"] == "error" for name in ROUTES))

    def test_receiver_identity_change_or_dead_receiver_invalidates_evidence(self):
        for field, value in (("pid", 402), ("window_id", 902), ("pid", 0), ("sequence", 0)):
            data = [sample(index=i) for i in range(2)]
            data[0]["receiver_after"][field] = value
            report = summarize(data, cleanup=CLEAN)
            self.assertEqual(report["baseline_status"], "unverified")
            self.assertEqual(report["phases"]["before_lock"]["receiver_status"], "unverified")
        data = [sample(index=i) for i in range(2)]
        for name in ("receiver_before", "receiver_after"):
            data[1][name]["pid"] = 402
        self.assertEqual(summarize(data, cleanup=CLEAN)["baseline_status"], "unverified")

    def test_receiver_restart_across_phases_and_phase_interleaving_are_unverified(self):
        restarted = trial()
        for item in restarted[4:]:
            for name in ("receiver_before", "receiver_after"):
                item[name]["pid"] = 402
        interleaved = trial()
        interleaved[1], interleaved[2] = interleaved[2], interleaved[1]
        for data in (restarted, interleaved):
            report = summarize(data, mode="lock", cleanup=CLEAN)
            self.assertEqual(report["phase_order_status"], "unverified")
            self.assertEqual(report["lock_status"], "unverified")
            self.assertEqual(report["recording_status"], "unverified")

    def test_missing_or_malformed_observation_prevents_completion(self):
        for invalid in (None, {}, {"status": "decoded", "sequence": None,
                                  "api_code": 0, "uptime": 100.5}):
            data = [sample(index=i) for i in range(2)]
            data[0]["observations"]["retained_ax"] = invalid
            report = summarize(data, cleanup=CLEAN)
            self.assertEqual(report["baseline_status"], "unverified")
            self.assertEqual(report["recording_status"], "unverified")

    def test_error_and_cleanup_failure_cannot_pass_even_with_fresh_routes(self):
        cases = (({"errors": ["failed acquisition"]}, "passed", 0),
                 ({"cleanup": {"status": "failed", "errors": []}}, "failed", 1),
                 ({"cleanup": {"status": "passed", "errors": ["receiver alive"]}}, "failed", 1),
                 ({"cleanup": None}, "unverified", 0))
        for kwargs, status, count in cases:
            args = {"cleanup": CLEAN, **kwargs}
            report = summarize(trial(), mode="lock", **args)
            self.assertNotEqual(report["baseline_status"], "passed")
            self.assertNotEqual(report["lock_status"], "passed")
            self.assertEqual(report["cleanup_status"], status)
            self.assertEqual(report["cleanup_error_count"], count)

    def test_aggregate_does_not_mutate_or_leak_raw_sample_data(self):
        data = trial()
        data[0].update(path="/private/user-lock-probe", raw_text="private receiver content")
        data[0]["observations"]["retained_ax"]["error"] = "private receiver content"
        original = copy.deepcopy(data)
        report = summarize(data, mode="lock", cleanup=CLEAN)
        self.assertEqual(data, original)
        encoded = json.dumps(report)
        for token in ("pid", "window_id", "/private", "raw_text", "private receiver content"):
            self.assertNotIn(token, encoded)

    def test_invalid_mode_and_unknown_phase_are_not_silent_success(self):
        with self.assertRaises(ValueError):
            summarize([], mode="other")
        data = [sample(index=i) for i in range(2)] + [{"phase": "unknown"}]
        report = summarize(data, cleanup=CLEAN)
        self.assertEqual(report["error_count"], 1)
        self.assertNotEqual(report["baseline_status"], "passed")

    def test_unhashable_phase_and_status_are_reported_instead_of_crashing(self):
        data = [sample(index=i) for i in range(2)] + [{"phase": []}]
        report = summarize(data, cleanup=CLEAN)
        self.assertEqual(report["error_count"], 1)
        data = [sample(index=i) for i in range(2)]
        data[0]["observations"]["retained_ax"]["status"] = []
        data[0]["observations"]["new_screenshot"]["status"] = []
        report = summarize(data, cleanup=CLEAN)
        self.assertEqual(report["recording_status"], "unverified")
        self.assertEqual(route(report)["outcome_counts"], {"error": 1, "matching": 1})


if __name__ == "__main__":
    unittest.main()
