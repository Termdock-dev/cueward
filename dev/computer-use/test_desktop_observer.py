"""Model, evidence and transport regressions; these are not desktop acceptance."""

import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from desktop_observer import DesktopObserver, execution_from_records, observation_checks
from task_acceptance.trace import check_trace

HERE = Path(__file__).resolve().parent


def records():
    result = [{"type": "ready", "schema": 1, "receiver_pids": [200]}]
    for ms in (0, 20, 40, 60, 80, 100):
        result.append({"type": "sample", "ms": ms, "frontmost_pid": 100, "pointer": [10, 20],
                       "visible_spaces": {"1": "2", "2": "3"}, "target_active": {"200": False},
                       "session_locked": False})
        if ms == 20:
            result.append({"type": "mark", "ms": 21, "id": "start"})
        if ms == 80:
            result.append({"type": "mark", "ms": 81, "id": "end"})
    return result


class EvidenceTests(unittest.TestCase):
    def execution(self):
        return execution_from_records(records(), "start", "end")

    def test_markers_use_observer_clock_and_bracket_samples(self):
        execution = self.execution()
        self.assertEqual((execution["start_ms"], execution["end_ms"]), (21, 81))
        self.assertEqual([s["ms"] for s in execution["samples"]], [20, 40, 60, 80, 100])
        self.assertEqual(observation_checks(execution, [200])["status"], "passed")

    def test_missing_boundary_or_duplicate_marker_is_rejected(self):
        for data in (records()[:-1], records() + [{"type": "mark", "ms": 90, "id": "end"}]):
            with self.subTest(data=data), self.assertRaises(ValueError):
                execution_from_records(data, "start", "end")

    def test_transient_activation_cannot_hide_between_samples(self):
        data = records() + [{"type": "activation", "ms": 50, "pid": 200}]
        execution = execution_from_records(data, "start", "end")
        self.assertEqual(observation_checks(execution, [200])["status"], "failed")
        status, _, _ = check_trace({"receiver_pids": [200], "execution": execution,
                                   "receipts": [{"command": "test", "elapsed_ms": 0, "status": "observation"}]})
        self.assertEqual(status, "failed")

    def test_observation_failure_or_lock_cannot_pass(self):
        execution = self.execution()
        execution["events"].append({"type": "observation_error", "ms": 30, "error": "missing second display"})
        self.assertEqual(observation_checks(execution, [200])["status"], "unverified")
        status, _, _ = check_trace({"receiver_pids": [200], "execution": execution,
                                   "receipts": [{"command": "test", "elapsed_ms": 0, "status": "observation"}]})
        self.assertEqual(status, "unverified")
        execution = self.execution()
        execution["samples"][1]["session_locked"] = True
        self.assertEqual(observation_checks(execution, [200])["status"], "unverified")

    def test_wrong_receiver_scope_cannot_pass(self):
        self.assertEqual(observation_checks(self.execution(), [201])["status"], "unverified")

    def test_foreground_space_pointer_changes_and_gaps_are_not_passed(self):
        for field, value in (("frontmost_pid", 101), ("pointer", [11, 20]), ("visible_spaces", {"1": "9"}), ("ms", 200)):
            execution = self.execution()
            execution["samples"][-1][field] = value
            with self.subTest(field=field):
                self.assertNotEqual(observation_checks(execution, [200])["status"], "passed")

    def test_physical_pointer_policy_retains_data_without_claiming_attribution(self):
        execution = self.execution()
        execution["samples"][1]["pointer"] = [30, 40]
        original = copy.deepcopy(execution)
        result = observation_checks(execution, [200], "physical_concurrent")
        self.assertEqual(result["status"], "unverified")
        self.assertIn("attribution", result["reason"])
        self.assertEqual(execution, original)
        execution["samples"][1]["target_active"]["200"] = True
        self.assertEqual(observation_checks(execution, [200], "physical_concurrent")["status"], "failed")


class SwiftModelTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="cueward-desktop-model-")
        directory = Path(cls.temp.name)
        cls.binary = directory / "ModelTests"
        source = directory / "main.swift"
        source.write_text((HERE / "desktop-observer-model.swift").read_text() + '''
let value = try JSONSerialization.jsonObject(with: FileHandle.standardInput.readDataToEndOfFile()) as! [String: Any]
do {
    var result: [String: Any] = [:]
    switch value["action"] as? String {
    case "config":
        let config = try ObserverConfiguration(value["value"] as! [String: Any])
        result = ["pids": config.pids, "interval": config.intervalMS]
    case "command":
        let command = try observerCommand(JSONSerialization.data(withJSONObject: value["value"]!))
        result = ["command": command.0, "id": command.1 as Any? ?? NSNull()]
    default:
        result = try correlateDisplaySpaces(value["rows"] as! [[String: Any]],
            online: value["online"] as! [String: String], mainUUID: value["main"] as! String)
    }
    print(String(data: try JSONSerialization.data(withJSONObject: ["result": result]), encoding: .utf8)!)
} catch {
    print(String(data: try JSONSerialization.data(withJSONObject: ["error": String(describing: error)]), encoding: .utf8)!)
}
''')
        subprocess.run(["swiftc", "-module-cache-path", str(directory / "cache"), str(source), "-o", str(cls.binary)],
                       check=True, capture_output=True, timeout=90)

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def run_model(self, value):
        output = subprocess.run([str(self.binary)], input=json.dumps(value), text=True,
                                capture_output=True, check=True, timeout=5)
        return json.loads(output.stdout)

    def display(self, rows=None, online=None):
        return {"action": "spaces", "main": "ABC", "online": online or {"1": "ABC", "2": "DEF"},
                "rows": rows if rows is not None else [
                    {"Display Identifier": "Main", "Current Space": {"ManagedSpaceID": 2}},
                    {"Display Identifier": "def", "Current Space": {"ManagedSpaceID": 3}}]}

    def test_all_online_displays_are_correlated_including_main_alias(self):
        self.assertEqual(self.run_model(self.display())["result"], {"1": "2", "2": "3"})

    def test_missing_duplicate_or_unreadable_display_is_rejected(self):
        valid = self.display()["rows"]
        for rows in (valid[:1], valid + valid[:1], [{}],
                     [{"Display Identifier": "Main", "Current Space": {"ManagedSpaceID": True}}]):
            with self.subTest(rows=rows):
                self.assertIn("error", self.run_model(self.display(rows=rows)))
        self.assertIn("error", self.run_model(self.display(online={"1": "ABC", "2": "ABC"})))

    def test_configuration_bounds_and_boolean_pids(self):
        value = {"pids": [200], "interval_ms": 20, "duration_ms": 1000}
        self.assertEqual(self.run_model({"action": "config", "value": value})["result"]["pids"], [200])
        for field, bad in (("pids", [True]), ("pids", [200, 200]), ("pids", []), ("interval_ms", 1), ("duration_ms", 1_800_001)):
            with self.subTest(field=field, bad=bad):
                self.assertIn("error", self.run_model({"action": "config", "value": {**value, field: bad}}))

    def test_unknown_or_oversized_control_messages_are_rejected(self):
        for value in ({"command": "activate"}, {"command": "mark", "id": ""}, {"command": "mark", "id": "x" * 4097}):
            self.assertIn("error", self.run_model({"action": "command", "value": value}))
        self.assertEqual(self.run_model({"action": "command", "value": {"command": "mark", "id": "task-start"}})["result"]["id"], "task-start")


class TransportTests(unittest.TestCase):
    def test_real_pipe_marker_acknowledgement_stop_and_exclusive_log(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            helper = root / "helper"
            helper.write_text(f"#!{sys.executable}\n" + '''
import json, sys
def sample(ms):
    return {"type":"sample", "ms":ms,"frontmost_pid":100,"pointer":[10,20],"visible_spaces":{"1":"2"},"target_active":{"200":False},"session_locked":False}
def emit(r): print(json.dumps(r), flush=True)
emit({"type":"ready", "schema":1,"receiver_pids":[200]});emit(sample(0))
ms=0
for line in sys.stdin:
    command=json.loads(line)
    if command["command"]=="stop": emit({"type":"finished","reason":"stopped"}); break
    ms+=20;emit(sample(ms));emit({"type":"mark","ms":ms+1,"id":command["id"]});emit(sample(ms+2))
''')
            helper.chmod(0o755)
            path = root / "trace.jsonl"
            with DesktopObserver(helper, [200], path) as observer:
                start, end = observer.mark(), observer.mark()
                execution = observer.execution(start, end)
                self.assertGreater(execution["end_ms"], execution["start_ms"])
            self.assertIsNotNone(observer.process.returncode)
            saved = [json.loads(line) for line in path.read_text().splitlines()]
            self.assertEqual(saved[-1]["type"], "finished")
            with self.assertRaises(FileExistsError):
                with DesktopObserver(helper, [200], path):
                    pass


class SwiftRunLoopTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="cueward-observer-loop-")
        root = Path(cls.temp.name)
        platform = root / "platform.swift"
        # Only replace the desktop read boundary. The actual protocol/run loop is executed.
        platform.write_text('''
import Foundation
@MainActor final class DesktopObservation {
    init(pids: [Int32]) throws {}
    func sample(ms: Double) throws -> [String: Any] {
        ["type":"sample", "ms":ms, "frontmost_pid":100, "pointer":[10,20],
         "visible_spaces":["1":"2"], "target_active":["200":false], "session_locked":false]
    }
}
''')
        cls.binary = root / "ObserverLoop"
        subprocess.run(["swiftc", "-swift-version", "6", "-warnings-as-errors",
                        "-module-cache-path", str(root / "cache"),
                        str(HERE / "desktop-observer-model.swift"), str(platform),
                        str(HERE / "desktop-observer-main.swift"), "-o", str(cls.binary)],
                       check=True, capture_output=True, timeout=90)

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_real_run_loop_marks_and_paired_boundary_samples(self):
        path = Path(self.temp.name) / "protocol.jsonl"
        with DesktopObserver(self.binary, [200], path, duration_ms=5000) as observer:
            start = observer.mark("start")
            import time
            time.sleep(0.06)
            end = observer.mark("end")
            execution = observer.execution(start, end)
            self.assertEqual(observation_checks(execution, [200])["status"], "passed")
        self.assertEqual(observer.records[-1]["reason"], "stopped")

    def test_deadline_without_controls_does_not_hang(self):
        configuration = {"pids": [200], "interval_ms": 20, "duration_ms": 100}
        process = subprocess.Popen([str(self.binary), json.dumps(configuration)], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            process.wait(timeout=3)
            data = [json.loads(line) for line in process.stdout.read().splitlines()]
            self.assertEqual(data[-1]["reason"], "deadline")
            self.assertGreaterEqual(sum(r["type"] == "sample" for r in data), 3)
        finally:
            if process.poll() is None:
                process.kill(); process.wait()
            for stream in (process.stdin, process.stdout, process.stderr):
                stream.close()

    def test_invalid_control_is_reported_and_stops_without_desktop_action(self):
        configuration = {"pids": [200], "interval_ms": 20, "duration_ms": 5000}
        output = subprocess.run([str(self.binary), json.dumps(configuration)],
                                input='{ "command": "activate" }\n', capture_output=True, text=True, timeout=3)
        data = [json.loads(line) for line in output.stdout.splitlines()]
        self.assertTrue(any(r["type"] == "control_error" for r in data))
        self.assertEqual(data[-1]["reason"], "control_error")


if __name__ == "__main__":
    unittest.main()
