"""A desktop change is held against Cueward only while one of its commands could have made it."""
import unittest

from desktop_observer import execution_from_records
from task_acceptance.trace import check_trace

STEP_MS, END_MS = 50, 4000


def evidence(command=(3000, 3500), marks=True):
    samples = [{"type": "sample", "ms": ms, "frontmost_pid": 100, "pointer": [10, 20],
                "visible_spaces": {"1": "2"}, "target_active": {"200": False}, "session_locked": False,
                "input": {"physical_pointer": 0, "automated_pointer": 0, "physical_other": 0}}
               for ms in range(0, END_MS + STEP_MS, STEP_MS)]
    execution = {"start_ms": 1, "end_ms": END_MS - 1, "samples": samples, "events": [],
                 "observer": {"schema": 1, "receiver_pids": [200]}}
    if marks:
        execution["marks"] = {"begin": command[0], "finish": command[1]}
    return {"execution": execution, "receiver_pids": [200],
            "receipts": [{"command": "cueward app press", "elapsed_ms": 1, "status": "sent_unverified",
                          "start_marker": "begin", "end_marker": "finish"}]}


def change(record, at, field, value):
    for sample in record["execution"]["samples"]:
        if sample["ms"] >= at:
            if field == "automated_pointer":
                sample["input"][field] += 1
            else:
                sample[field] = value


def drop(record, first, last):
    execution = record["execution"]
    execution["samples"] = [s for s in execution["samples"] if not first <= s["ms"] <= last]


class CommandAttributionTests(unittest.TestCase):
    def status(self, record):
        status, reason, metrics = check_trace(record)
        return status, reason, metrics

    def test_foreground_change_with_no_command_running_is_external(self):
        record = evidence()
        change(record, 500, "frontmost_pid", 300)
        status, _, metrics = self.status(record)
        self.assertEqual(status, "passed")
        self.assertEqual(metrics["external_desktop_changes"], 1)

    def test_foreground_change_during_a_command_fails(self):
        record = evidence()
        change(record, 3200, "frontmost_pid", 300)
        status, reason, _ = self.status(record)
        self.assertEqual(status, "failed")
        self.assertIn("without physical input", reason)

    def test_foreground_change_just_after_a_command_fails(self):
        record = evidence()
        change(record, 3900, "frontmost_pid", 300)
        self.assertEqual(self.status(record)[0], "failed")

    def test_foreground_change_long_after_a_command_is_external(self):
        record = evidence(command=(500, 800))
        change(record, 3000, "frontmost_pid", 300)
        self.assertEqual(self.status(record)[0], "passed")

    def test_change_just_before_a_command_starts_is_external(self):
        record = evidence()
        change(record, 2950, "frontmost_pid", 300)
        self.assertEqual(self.status(record)[0], "passed")

    def test_pointer_and_automated_input_follow_the_same_rule(self):
        for field, value in (("pointer", [30, 40]), ("automated_pointer", None)):
            with self.subTest(field=field):
                outside, inside = evidence(), evidence()
                change(outside, 500, field, value)
                change(inside, 3200, field, value)
                self.assertEqual(self.status(outside)[0], "passed")
                self.assertEqual(self.status(inside)[0], "failed")

    def test_receiver_in_the_foreground_fails_with_no_command_running(self):
        record = evidence()
        change(record, 500, "frontmost_pid", 200)
        status, reason, _ = self.status(record)
        self.assertEqual(status, "failed")
        self.assertIn("receiver", reason)

    def test_receiver_active_fails_with_no_command_running(self):
        record = evidence()
        change(record, 500, "target_active", {"200": True})
        self.assertEqual(self.status(record)[0], "failed")

    def test_evidence_without_marks_stays_strict(self):
        record = evidence(marks=False)
        change(record, 500, "frontmost_pid", 300)
        status, _, metrics = self.status(record)
        self.assertEqual(status, "failed")
        self.assertNotIn("external_desktop_changes", metrics)

    def test_a_receipt_without_markers_stays_strict(self):
        record = evidence()
        del record["receipts"][0]["end_marker"]
        change(record, 500, "frontmost_pid", 300)
        self.assertEqual(self.status(record)[0], "failed")

    def test_sample_gap_with_no_command_running_does_not_block_a_result(self):
        record = evidence()
        drop(record, 500, 800)
        status, _, metrics = self.status(record)
        self.assertEqual(status, "passed")
        self.assertEqual(metrics["observer_max_interval_ms"], 350)
        self.assertEqual(metrics["observer_max_command_interval_ms"], 50)

    def test_sample_gap_during_a_command_stays_unverified(self):
        record = evidence()
        drop(record, 3100, 3400)
        status, reason, _ = self.status(record)
        self.assertEqual(status, "unverified")
        self.assertIn("gap", reason)

    def test_sample_gap_just_after_a_command_stays_unverified(self):
        record = evidence(command=(1000, 1500))
        drop(record, 2300, 2450)
        self.assertEqual(self.status(record)[0], "unverified")

    def test_sample_gap_without_marks_stays_unverified(self):
        record = evidence(marks=False)
        drop(record, 500, 800)
        self.assertEqual(self.status(record)[0], "unverified")

    def test_execution_keeps_the_observer_marks(self):
        rows = [{"type": "ready", "schema": 1}]
        for ms in (0, 100):
            rows.append({"type": "sample", "ms": ms})
        rows[2:2] = [{"type": "mark", "ms": 10, "id": "start"}, {"type": "mark", "ms": 20, "id": "x"},
                     {"type": "mark", "ms": 30, "id": "end"}]
        self.assertEqual(execution_from_records(rows, "start", "end")["marks"],
                         {"start": 10, "x": 20, "end": 30})


if __name__ == "__main__":
    unittest.main()
