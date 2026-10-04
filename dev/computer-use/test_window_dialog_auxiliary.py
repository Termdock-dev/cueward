"""Synthetic auxiliary-panel scope/coverage tests; shared services are not owned."""
from copy import deepcopy
import tempfile
import unittest

from task_acceptance.evaluate import evaluate, load_json
from task_acceptance.setup import EDITED, prepare
from task_acceptance.window_dialog import check_auxiliary_receiver_trace, check_window_dialog
from test_window_dialog_native import binding, state


class DialogAuxiliaryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.run = prepare(self.temp.name)
        self.manifest = load_json(self.run / "manifest.json")
        self.artifact = self.run / "work/window_dialog/target.txt"
        self.artifact.write_text(EDITED)

    def record(self, *, auxiliary=True, service_root=True):
        final = state(self.artifact)
        final["contents"] = final["documents"][0]["contents"] = EDITED
        observations = []
        for index, phase in enumerate(("initial", "dialog", "resumed")):
            bound = binding(self.artifact, phase, reverse=phase != "initial")
            dialog = bound["dialog"]
            if dialog is not None and (auxiliary or service_root):
                if service_root:
                    dialog["root"]["receiver_pid"] = 200
                dialog["native_binding"] = {"source": "native_ax_window_binding", "window_id": 30,
                    "owner_window": {"pid": 100, "window_id": 20}, "owner_window_cf_equal": True,
                    "receiver_pids": [100, 200]}
            observations.append({"phase": phase, "snapshot_id": phase, "observed_ms": 20 + index * 20,
                "target_file": str(self.artifact), "windows": bound["windows"], "dialog": dialog,
                "active_root": deepcopy(dialog["root"] if dialog else bound["windows"][0]["root"])})
        observer = final | {"source": "receiver_observer", "run_id": self.manifest["run_id"],
            "task_id": "window_dialog", "window_observations": observations, "native_io_complete": True}
        if auxiliary:
            observer["auxiliary_receivers"] = [{"pid": 200, "window_id": 30,
                "owner_window": {"pid": 100, "window_id": 20}, "source": "native_ax_window_binding"}]
        receivers = [100, 200] if auxiliary else [100]
        samples = [{"ms": ms, "frontmost_pid": 99, "pointer": [0, 0], "visible_spaces": {"one": "A"},
                    "target_active": {str(pid): False for pid in receivers}, "session_locked": False}
                   for ms in (0, 50, 100)]
        return {"run_id": self.manifest["run_id"], "task_id": "window_dialog", "attempted": True,
            "receiver_pids": [100], "observer": observer,
            "receipts": [{"command": "synthetic observation", "elapsed_ms": 1, "status": "confirmed"}],
            "execution": {"start_ms": 10, "end_ms": 90, "samples": samples, "events": [],
                          "observer": {"schema": 1, "receiver_pids": receivers}}}

    def transition(self, record):
        return check_window_dialog(self.run, record, record["observer"])[0]

    def auxiliary_trace(self, record):
        return check_auxiliary_receiver_trace(self.run, record, record["observer"])[0]

    def result(self, record):
        report = evaluate(self.run, {"schema": 1, "run_id": self.manifest["run_id"], "kind": "agent",
                                     "tasks": {"window_dialog": record}})
        return next(task for task in report["tasks"] if task["task_id"] == "window_dialog")

    def test_native_service_root_and_descendant_bindings_both_pass(self):
        for service_root in (True, False):
            with self.subTest(service_root=service_root):
                record = self.record(service_root=service_root)
                before = deepcopy(record)
                self.assertEqual(self.transition(record), "passed")
                self.assertEqual(self.auxiliary_trace(record), "passed")
                self.assertEqual(self.result(record)["status"], "passed")
                self.assertEqual(record, before, "trace scoping must not mutate evidence or ownership")
                self.assertEqual(record["receiver_pids"], [100])

    def test_multiple_service_descendants_can_share_only_the_exact_bound_panel(self):
        record = self.record(service_root=False)
        observer = record["observer"]
        observer["auxiliary_receivers"].append(observer["auxiliary_receivers"][0] | {"pid": 201})
        observer["window_observations"][1]["dialog"]["native_binding"]["receiver_pids"].append(201)
        record["execution"]["observer"]["receiver_pids"] = [201, 100, 200]
        for sample in record["execution"]["samples"]:
            sample["target_active"]["201"] = False
        self.assertEqual(self.result(record)["status"], "passed")
        observer["auxiliary_receivers"][1]["window_id"] = 31
        self.assertEqual(self.transition(record), "failed")

    def test_foreign_duplicate_unbound_or_contradictory_auxiliary_entries_fail(self):
        changes = [
            lambda o: o["auxiliary_receivers"][0].update(pid=999),
            lambda o: o["auxiliary_receivers"][0].update(pid=100),
            lambda o: o["auxiliary_receivers"][0].update(pid=True),
            lambda o: o["auxiliary_receivers"][0].update(window_id=31),
            lambda o: o["auxiliary_receivers"][0].update(window_id=20),
            lambda o: o["auxiliary_receivers"][0].update(source="agent token"),
            lambda o: o["auxiliary_receivers"][0].update(owner_window={"pid": 100, "window_id": 21}),
            lambda o: o["auxiliary_receivers"][0].update(owner_window={"pid": 999, "window_id": 20}),
            lambda o: o["auxiliary_receivers"].append(deepcopy(o["auxiliary_receivers"][0])),
            lambda o: o["auxiliary_receivers"].append(o["auxiliary_receivers"][0] | {"pid": 201}),
            lambda o: o.update(auxiliary_receivers={}),
        ]
        for index, change in enumerate(changes):
            record = self.record()
            change(record["observer"])
            with self.subTest(index=index):
                self.assertEqual(self.transition(record), "failed")

    def test_missing_auxiliary_and_native_identity_fields_are_unverified(self):
        changes = [lambda o, key=key: o["auxiliary_receivers"][0].pop(key)
                   for key in ("pid", "window_id", "owner_window", "source")]
        changes += [lambda o: o["auxiliary_receivers"][0]["owner_window"].pop("window_id")]
        changes += [lambda o, key=key: o["window_observations"][1]["dialog"].pop(key)
                    for key in ("window_id", "native_binding", "parent_root")]
        changes += [lambda o, key=key: o["window_observations"][1]["dialog"]["native_binding"].pop(key)
                    for key in ("source", "window_id", "owner_window", "owner_window_cf_equal", "receiver_pids")]
        changes += [lambda o, key=key: o["window_observations"][1]["dialog"]["root"].pop(key)
                    for key in ("receiver_pid", "ref", "role")]
        for index, change in enumerate(changes):
            record = self.record()
            change(record["observer"])
            with self.subTest(index=index):
                self.assertEqual(self.transition(record), "unverified")

    def test_native_proof_contradictions_fail_and_unknown_owner_equality_stays_unverified(self):
        for change in ({"source": "title"}, {"window_id": 31},
                       {"owner_window": {"pid": 100, "window_id": 21}}, {"owner_window_cf_equal": False},
                       {"receiver_pids": [100]}, {"receiver_pids": [100, 200, 201]},
                       {"receiver_pids": [100, 200, 200]}, {"receiver_pids": [True, 200]},
                       {"receiver_pids": None}):
            record = self.record()
            record["observer"]["window_observations"][1]["dialog"]["native_binding"].update(change)
            with self.subTest(change=change):
                self.assertEqual(self.transition(record), "failed")
        record = self.record()
        record["observer"]["window_observations"][1]["dialog"]["native_binding"]["owner_window_cf_equal"] = None
        self.assertEqual(self.transition(record), "unverified")

    def test_service_needs_binding_even_when_root_is_host_and_service_only_in_descendants(self):
        for service_root in (True, False):
            for missing in (True, False):
                record = self.record(service_root=service_root)
                if missing:
                    record["observer"].pop("auxiliary_receivers")
                else:
                    record["observer"]["auxiliary_receivers"] = []
                with self.subTest(service_root=service_root, missing=missing):
                    self.assertEqual(self.transition(record), "failed")

    def test_document_root_never_accepts_shared_service_and_main_ownership_cannot_expand(self):
        for index in (0, 1):
            record = self.record()
            record["observer"]["window_observations"][0]["windows"][index]["root"]["receiver_pid"] = 200
            self.assertEqual(self.transition(record), "failed")
        record = self.record()
        record["receiver_pids"].append(200)
        self.assertEqual(self.transition(record), "failed")

    def test_proved_service_sheet_still_requires_current_owner_hierarchy(self):
        for change in ({"receiver_pid": 200}, {"ref": "w0"}):
            record = self.record()
            record["observer"]["window_observations"][1]["dialog"]["parent_root"].update(change)
            self.assertEqual(self.transition(record), "failed")
        record = self.record()
        record["observer"]["window_observations"][1]["active_root"]["ref"] = "w0.0"
        self.assertEqual(self.transition(record), "failed")

    def test_standalone_shared_service_panel_remains_unverified(self):
        record = self.record()
        phase = record["observer"]["window_observations"][1]
        phase["dialog"].update(mode="window", root={"receiver_pid": 200, "ref": "w2", "role": "AXWindow"})
        phase["dialog"]["native_binding"].update(owner_window_cf_equal=None, owner_source="NSDocument.prepareSavePanel")
        phase["active_root"] = deepcopy(phase["dialog"]["root"])
        self.assertEqual(self.transition(record), "unverified")

    def test_legacy_host_only_sheet_contract_is_unchanged(self):
        record = self.record(auxiliary=False, service_root=False)
        record["observer"]["window_observations"][1]["dialog"].pop("window_id")
        result = self.result(record)
        self.assertEqual(result["status"], "passed")
        self.assertNotIn("auxiliary_receiver_trace", {check["name"] for check in result["checks"]})

    def test_missing_or_late_service_trace_cannot_be_repaired_by_successful_artifact(self):
        changes = [
            lambda e: e["observer"].update(receiver_pids=[100]),
            lambda e: e.pop("observer"),
            lambda e: e["observer"].update(receiver_pids=[100, 200, 999]),
            lambda e: e["observer"].update(receiver_pids=[100, 200, 200]),
            lambda e: e["samples"][0]["target_active"].pop("200"),
            lambda e: e["samples"][0]["target_active"].update({"200": None}),
            lambda e: e["samples"][0]["target_active"].update({"200": 0}),
            lambda e: e["samples"][0].update(ms=20),
            lambda e: e["samples"].pop(0),
            lambda e: e["samples"][1].update(ms=0),
            lambda e: e["samples"][-1].update(ms=200),
            lambda e: e["samples"][-1].update(ms=80),
            lambda e: e.pop("events"),
            lambda e: e["events"].append({"type": "observation_error", "ms": 20}),
            lambda e: e["events"].append({"type": "activation", "ms": 20}),
            lambda e: e["samples"][0].update(session_locked=True),
        ]
        for index, change in enumerate(changes):
            record = self.record()
            change(record["execution"])
            with self.subTest(index=index):
                self.assertEqual(self.auxiliary_trace(record), "unverified")
                result = self.result(record)
                self.assertEqual(result["status"], "unverified", result)
                self.assertIn({"name": "artifact", "status": "passed", "reason": "exact UTF-8 byte comparison"},
                              result["checks"])

    def test_service_activation_is_failed_even_with_other_coverage_missing(self):
        for active_event in (False, True):
            record = self.record()
            record["execution"]["observer"]["receiver_pids"] = [100]
            if active_event:
                record["execution"]["events"].append({"type": "activation", "ms": 30, "pid": 200})
            else:
                record["execution"]["samples"][1]["target_active"]["200"] = True
            self.assertEqual(self.auxiliary_trace(record), "failed")
            self.assertEqual(self.result(record)["status"], "failed")

    def test_complete_auxiliary_coverage_does_not_filter_other_interference(self):
        for change in ({"frontmost_pid": 200}, {"pointer": [1, 0]}, {"visible_spaces": {"one": "B"}},
                       {"target_active": {"100": True, "200": False}}):
            record = self.record()
            record["execution"]["samples"][1].update(change)
            self.assertEqual(self.auxiliary_trace(record), "passed")
            result = self.result(record)
            self.assertEqual(result["status"], "failed", result)
            self.assertEqual(next(check for check in result["checks"] if check["name"] == "interference_trace")["status"],
                             "failed")


if __name__ == "__main__":
    unittest.main()
