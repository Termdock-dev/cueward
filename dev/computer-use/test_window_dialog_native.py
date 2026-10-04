"""Owned scope and independent window/dialog transitions; no desktop acceptance claims."""
from copy import deepcopy
import json
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import Mock

from agent_session import ScopeError
from task_acceptance.evaluate import evaluate, load_json
from task_acceptance.setup import EDITED, ORIGINAL, prepare, write_json
from test_agent_session import token
from window_dialog_observer import owned_inventory, scoped_dialog, validate_native_result
from window_dialog_session import DialogSession, command_options
from window_dialog_task import dialog_evidence
from native_agent_task import task_setup


def binding(artifact, phase="initial", reverse=False):
    paths = [artifact, artifact.with_name("bystander.txt")]
    windows = [{"pid": 100, "window_id": 20 + i, "file": str(p), "ax_identifier": "unique-" + str(i),
                "root": {"receiver_pid": 100, "ref": "w" + str(1-i if reverse else i), "role": "AXWindow"}}
               for i, p in enumerate(paths)]
    dialog = None
    if phase == "dialog":
        parent = windows[0]["root"]
        dialog = {"kind": "save", "mode": "sheet", "window_id": 30,
                  "owner_window": {"pid": 100, "window_id": 20}, "ax_identifier": "save-unique",
                  "root": {"receiver_pid": 100, "ref": parent["ref"] + ".0", "role": "AXSheet"}, "parent_root": parent}
    return {"pid": 100, "windows": windows, "dialog": dialog}


def state(artifact):
    b = binding(artifact)
    documents = [{k: v for k, v in w.items() if k != "root"} | {
        "contents": ORIGINAL if i == 0 else "Preserve this synthetic file.\n", "initial_contents": ORIGINAL if i == 0 else "Preserve this synthetic file.\n",
        "read_requests": 1, "save_requests": 1 if i == 0 else 0, "data_requests": 1 if i == 0 else 0,
        "error": "", "document_edited": False, "rejected_requests": 0}
        for i, w in enumerate(b["windows"])]
    return documents[0] | {"pid": 100, "owned_windows": [{k: v for k, v in w.items() if k != "root"} for w in b["windows"]],
                           "documents": documents, "active": False, "activations": 0, "observation_sequence": 12,
                           "fresh_after_sequence": 11, "dialog": None}


class DialogNativeTests(unittest.TestCase):
    def make_session(self, folder, artifact, snapshots):
        observer = Mock(condition=threading.Condition(), records=[])
        def mark(name):
            observer.records.append({"type": "mark", "id": name, "ms": 10 + len(observer.records) * 10})
            return name
        observer.mark.side_effect = mark
        session = DialogSession("/mock/cueward", observer, 100, 20, folder / "session.json", {},
                                bindings=Mock(read=Mock(side_effect=snapshots)), artifact=artifact)
        return session

    def inspect_result(self, b, ref=None, target=False):
        nodes = [{"ref": w["root"]["ref"], "identifier": w["ax_identifier"], "role": "AXWindow", "receiver_pid": 100}
                 for w in b["windows"] if ref is None or w["root"]["ref"] == ref]
        if target:
            ref = ref + ".1"
            nodes.append({"ref": ref, "receiver_pid": 100,
                          "target": token({"kind": "app_ax", "app": {"pid": 100}, "ref": ref})})
        return {"app": {"pid": 100}, "nodes": nodes}

    def test_setup_checks_both_baselines_and_keeps_two_existing_files(self):
        with tempfile.TemporaryDirectory() as parent:
            run = prepare(parent)
            _, _, artifact, output = task_setup(run, "window_dialog")
            self.assertEqual(artifact.read_text(), ORIGINAL)
            self.assertEqual(set(p.name for p in artifact.parent.iterdir()), {"target.txt", "bystander.txt"})
            self.assertEqual(output, run / "operator/window_dialog")
            other = prepare(parent)
            (other / "work/window_dialog/bystander.txt").write_text("changed")
            with self.assertRaisesRegex(ValueError, "baseline"):
                task_setup(other, "window_dialog")
            self.assertFalse((other / "operator").exists())

    def test_native_inventory_rejects_duplicate_or_foreign_window_identity(self):
        artifact = Path("/run/work/window_dialog/target.txt")
        original = state(artifact)
        self.assertEqual(len(owned_inventory(original, 100, artifact)), 2)
        for key, value in (("window_id", 20), ("file", str(artifact)), ("pid", 101), ("ax_identifier", "unique-0")):
            bad = deepcopy(original); bad["owned_windows"][1][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                owned_inventory(bad, 100, artifact)

    def test_panel_folder_creation_scope_is_required_before_any_binding_dispatch(self):
        artifact = Path("/run/work/window_dialog/target.txt")
        original = state(artifact)
        windows = owned_inventory(original, 100, artifact)
        dialog = binding(artifact, "dialog")["dialog"] | {
            "can_create_directories": False, "owner_binding": "NSDocument.prepareSavePanel"}
        self.assertEqual(scoped_dialog(original | {"dialog": dialog}, windows), dialog)
        for change in ({"can_create_directories": True}, {"can_create_directories": None},
                       {"owner_binding": "guessed title"}, {"owner_window": {"pid": 100, "window_id": 21}}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                scoped_dialog(original | {"dialog": dialog | change}, windows)

    def test_flag_parser_does_not_scan_text_values_and_rejects_escape_spellings(self):
        value = ["app", "set-value", "--target", "token", "--value", "--output=plain text"]
        self.assertEqual(command_options(value)["--value"], "--output=plain text")
        for args in (["app", "inspect", "--pid", "100", "--root=menu"],
                     ["app", "inspect", "--pid", "100", "--pid", "101"],
                     ["window", "snapshot", "--id", "20", "--output=/user/file"],
                     ["window", "click", "--target", "anything"], ["app", "open", "--file", "/user"]):
            with self.subTest(args=args), self.assertRaises(ScopeError):
                command_options(args)

    def test_menu_foreign_pid_and_unseen_target_never_dispatch(self):
        with tempfile.TemporaryDirectory() as folder:
            artifact = Path(folder) / "target.txt"; b = binding(artifact)
            session = self.make_session(Path(folder), artifact, [b] * 3)
            session.commands.call = Mock()
            for args in (["app", "inspect", "--pid", "100", "--root", "menu"],
                         ["app", "inspect", "--pid", "101"],
                         ["app", "press", "--target", token({"kind": "app_ax", "app": {"pid": 100}, "ref": "w0.1"})]):
                self.assertTrue(session.request(args)["not_dispatched"])
            session.commands.call.assert_not_called()

    def test_both_native_windows_are_readable_but_third_window_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            artifact = Path(folder) / "target.txt"; b = binding(artifact)
            session = self.make_session(Path(folder), artifact, [b] * 5)
            session.commands.call = Mock(return_value={"window_id": 21})
            self.assertIn("result", session.request(["window", "snapshot", "--id", "21"]))
            self.assertTrue(session.request(["window", "snapshot", "--id", "22"])["not_dispatched"])
            session.commands.call.assert_called_once()

    def test_root_reorder_invalidates_previously_returned_token_without_replay(self):
        with tempfile.TemporaryDirectory() as folder:
            artifact = Path(folder) / "target.txt"; b = binding(artifact); changed = binding(artifact, reverse=True)
            session = self.make_session(Path(folder), artifact, [b, b, changed])
            result = self.inspect_result(b, "w0", target=True)
            session.commands.call = Mock(return_value=result)
            session.request(["app", "inspect", "--pid", "100", "--root", "w0"])
            stale = result["nodes"][-1]["target"]
            self.assertTrue(session.request(["app", "press", "--target", stale])["not_dispatched"])
            session.commands.call.assert_called_once()

    def test_auxiliary_scope_requires_native_id_attachment_and_actual_receiver_proof(self):
        artifact = Path("/owned/target.txt"); b = binding(artifact, "dialog")
        owner = {"pid": 100, "window_id": 20}
        b["dialog"]["native_binding"] = {"source": "native_ax_window_binding", "window_id": 30,
            "owner_window": owner, "owner_window_cf_equal": True, "receiver_pids": [100, 200]}
        b["auxiliary_receivers"] = [{"pid": 200, "window_id": 30, "owner_window": owner,
                                     "source": "native_ax_window_binding"}]
        request = {"pid": 100, "target_file": str(artifact), "dialog": b["dialog"]}
        validate_native_result(b, request)
        for change in ({"window_id": 31}, {"owner_window_cf_equal": False}, {"receiver_pids": [100]},
                       {"receiver_pids": [True, 200]}, {"source": "agent_token"}):
            bad = deepcopy(b); bad["dialog"]["native_binding"].update(change)
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                validate_native_result(bad, request)
        bad = deepcopy(b); bad["windows"][0]["root"]["receiver_pid"] = 200
        with self.assertRaisesRegex(RuntimeError, "foreign native document"):
            validate_native_result(bad, request)

    def test_service_tokens_bind_only_the_independently_observed_owned_panel(self):
        with tempfile.TemporaryDirectory() as folder:
            artifact = Path(folder) / "target.txt"; b = binding(artifact, "dialog")
            b["dialog"]["root"]["receiver_pid"] = 200
            b["auxiliary_receivers"] = [{"pid": 200, "window_id": 30,
                "owner_window": {"pid": 100, "window_id": 20}, "source": "native_ax_window_binding"}]
            session = self.make_session(Path(folder), artifact, [b, b, b])
            token_value = token({"kind": "app_ax", "app": {"pid": 100}, "ref": "w0.0.1"})
            result = {"app": {"pid": 100}, "nodes": [{"ref": "w0.0.1", "receiver_pid": 200, "target": token_value}]}
            session.register(["app", "inspect", "--pid", "100", "--root", "w0.0"], result, b, b)
            self.assertIn(token_value, session.tokens)
            foreign = {"app": {"pid": 100}, "nodes": [{"ref": "w1.1", "receiver_pid": 200, "target":
                token({"kind": "app_ax", "app": {"pid": 100}, "ref": "w1.1"})}]}
            with self.assertRaisesRegex(RuntimeError, "owned receiver"):
                session.register(["app", "inspect", "--pid", "100", "--root", "w1"], foreign, b, b)

    def test_panel_receiver_is_checked_even_when_node_has_no_action_target(self):
        with tempfile.TemporaryDirectory() as folder:
            artifact = Path(folder) / "target.txt"; b = binding(artifact, "dialog")
            session = self.make_session(Path(folder), artifact, [b] * 3)
            button = {"ref": "w0.0.1", "receiver_pid": 100,
                      "target": token({"kind": "app_ax", "app": {"pid": 100}, "ref": "w0.0.1"})}
            args = ["app", "inspect", "--pid", "100", "--root", "w0.0"]
            for receiver in (200, None, 0):
                result = {"app": {"pid": 100}, "nodes": [
                    {"ref": "w0.0.0", "receiver_pid": receiver, "role": "AXGroup"}, button]}
                with self.subTest(receiver=receiver), self.assertRaisesRegex(RuntimeError, "unbound panel receiver"):
                    session.register(args, result, b, b)
            allowed = deepcopy(b)
            allowed["auxiliary_receivers"] = [{"pid": 200, "window_id": 30,
                "owner_window": {"pid": 100, "window_id": 20}, "source": "native_ax_window_binding"}]
            result = {"app": {"pid": 100}, "nodes": [
                {"ref": "w0.0.0", "receiver_pid": 200, "role": "AXGroup"}, button]}
            session.register(args, result, allowed, allowed)
            self.assertIn(button["target"], session.tokens)

    def test_binding_change_during_inspection_discards_phase_and_targets(self):
        with tempfile.TemporaryDirectory() as folder:
            artifact = Path(folder) / "target.txt"; b = binding(artifact); changed = binding(artifact, reverse=True)
            session = self.make_session(Path(folder), artifact, [b, changed])
            session.commands.call = Mock(return_value=self.inspect_result(b, "w0", target=True))
            session.request(["app", "inspect", "--pid", "100", "--root", "w0"])
            self.assertEqual(session.phases, [])
            self.assertEqual(session.tokens, {})

    def test_cli_root_uuid_mismatch_fails_after_dispatch_without_false_not_dispatched(self):
        with tempfile.TemporaryDirectory() as folder:
            artifact = Path(folder) / "target.txt"; b = binding(artifact)
            session = self.make_session(Path(folder), artifact, [b, b])
            result = self.inspect_result(b, "w0"); result["nodes"][0]["identifier"] = "other-window"
            session.commands.call = Mock(return_value=result)
            with self.assertRaisesRegex(RuntimeError, "disagrees"):
                session.request(["app", "inspect", "--pid", "100", "--root", "w0"])
            self.assertEqual(session.records[0]["result"], result)
            self.assertEqual(session.phases, [])

    def test_post_dispatch_bad_token_is_unknown_evidence_not_a_no_dispatch_response(self):
        with tempfile.TemporaryDirectory() as folder:
            artifact = Path(folder) / "target.txt"; b = binding(artifact)
            session = self.make_session(Path(folder), artifact, [b, b])
            result = self.inspect_result(b, "w0", target=True); result["nodes"][-1]["target"] = "invalid!"
            session.commands.call = Mock(return_value=result)
            with self.assertRaisesRegex(RuntimeError, "do not replay"):
                session.request(["app", "inspect", "--pid", "100", "--root", "w0"])
            session.commands.call.assert_called_once()
            self.assertEqual(session.records[0]["result"], result)

    def test_real_three_phase_root_changes_are_retained_in_observer_timebase(self):
        with tempfile.TemporaryDirectory() as folder:
            artifact = Path(folder) / "target.txt"
            initial, dialog, resumed = binding(artifact), binding(artifact, "dialog", True), binding(artifact, reverse=True)
            session = self.make_session(Path(folder), artifact, [initial, initial, dialog, dialog, resumed, resumed])
            session.commands.call = Mock(side_effect=[self.inspect_result(initial, "w0"),
                                                      {"app": {"pid": 100}, "nodes": []}, self.inspect_result(resumed, "w1")])
            for ref in ("w0", "w1.0", "w1"):
                session.request(["app", "inspect", "--pid", "100", "--root", ref])
            phases = session.phases
            self.assertEqual([p["phase"] for p in phases], ["initial", "dialog", "resumed"])
            self.assertEqual([p["observed_ms"] for p in phases], [10, 20, 30])
            self.assertEqual(phases[1]["dialog"]["owner_window"], {"pid": 100, "window_id": 20})
            self.assertEqual(phases[1]["dialog"]["parent_root"], phases[1]["windows"][0]["root"])
            self.assertEqual(load_json(session.phase_path)["window_observations"], phases)

    def test_disk_success_cannot_override_incomplete_native_io_or_late_activation(self):
        with tempfile.TemporaryDirectory() as parent:
            run = prepare(parent); manifest = load_json(run / "manifest.json")
            artifact = run / "work/window_dialog/target.txt"; artifact.write_text(EDITED)
            final = state(artifact); windows = final["owned_windows"]
            phases = []
            for i, phase in enumerate(("initial", "dialog", "resumed")):
                b = binding(artifact, phase)
                phases.append({"phase": phase, "snapshot_id": phase, "observed_ms": 20 + 20*i,
                               "target_file": str(artifact), "windows": b["windows"],
                               "active_root": b["dialog"]["root"] if b["dialog"] else b["windows"][0]["root"], "dialog": b["dialog"]})
            session = {"run_id": manifest["run_id"], "task_id": "window_dialog", "attempted": True, "receiver_pids": [100],
                       "receipts": [{"command": "actual", "elapsed_ms": 1, "status": "confirmed"}],
                       "execution": {"start_ms": 10, "end_ms": 90, "samples": [
                           {"ms": t, "frontmost_pid": 99, "pointer": [0, 0], "visible_spaces": {"one": "A"}, "target_active": {"100": False}}
                           for t in (0, 50, 100)]}}
            for change, check in (({}, None), ({"activations": 1}, "final_receiver_inactive"),
                                  ({"fresh_after_sequence": 12}, "final_receiver_inactive")):
                task = dialog_evidence(session, final | change, manifest, 100, artifact, windows, phases)
                report = evaluate(run, {"schema": 1, "run_id": manifest["run_id"], "kind": "agent", "tasks": {"window_dialog": task}})
                result = next(t for t in report["tasks"] if t["task_id"] == "window_dialog")
                if check is None:
                    self.assertEqual(result["status"], "passed", result)
                else:
                    self.assertNotEqual(result["status"], "passed", result)
            final["documents"][0]["data_requests"] = 0
            task = dialog_evidence(session, final, manifest, 100, artifact, windows, phases)
            self.assertFalse(task["observer"]["native_io_complete"])
            report = evaluate(run, {"schema": 1, "run_id": manifest["run_id"], "kind": "agent", "tasks": {"window_dialog": task}})
            result = next(t for t in report["tasks"] if t["task_id"] == "window_dialog")
            self.assertEqual(result["status"], "failed", result)


if __name__ == "__main__":
    unittest.main()
