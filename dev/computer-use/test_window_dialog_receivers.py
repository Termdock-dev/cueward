"""Run the pure Swift sheet-receiver boundary, without AppKit, AX or TCC."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
HARNESS = r'''
import Foundation

struct ReceiverInput: Decodable {
    struct Observation: Decodable {
        let ref: String
        let pid: Int32
        let nativeWindowID: UInt32?
    }
    let observations: [Observation]
    let hostPID: Int32
    let root: String
    let panelID: UInt32
    let alive: [Int32]
}

@main struct ReceiverCheck {
    static func main() {
        let output: [String: Any]
        do {
            let input = try JSONDecoder().decode(ReceiverInput.self,
                                                from: FileHandle.standardInput.readDataToEndOfFile())
            let samples = input.observations.map {
                DialogReceiverObservation(ref: $0.ref, pid: $0.pid, nativeWindowID: $0.nativeWindowID)
            }
            let receivers = try boundDialogSheetReceivers(samples, hostPID: input.hostPID,
                root: input.root, panelID: input.panelID, isAlive: { input.alive.contains($0) })
            output = ["status": "bound", "receiver_pids": receivers]
        } catch {
            output = ["status": "rejected", "error": String(describing: error)]
        }
        do {
            let data = try JSONSerialization.data(withJSONObject: output)
            FileHandle.standardOutput.write(data)
        } catch {
            FileHandle.standardError.write(Data("cannot encode receiver-test result\n".utf8))
            exit(2)
        }
    }
}
'''


def observation(ref, pid, native=30):
    return {"ref": ref, "pid": pid, "nativeWindowID": native}


class SheetReceiverTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="cueward-sheet-receivers-")
        cls.addClassCleanup(cls.temp.cleanup)
        folder = Path(cls.temp.name)
        harness = folder / "ReceiverCheck.swift"
        harness.write_text(HARNESS)
        cls.binary = folder / "ReceiverCheck"
        subprocess.run(["swiftc", "-swift-version", "6", "-parse-as-library",
                        "-strict-concurrency=complete", "-warnings-as-errors",
                        "-module-cache-path", str(folder / "cache"),
                        str(HERE / "window-dialog-receivers.swift"), str(harness), "-o", str(cls.binary)],
                       check=True, capture_output=True, timeout=90)

    def request(self, observations=None, **change):
        payload = {"observations": observations if observations is not None else [
            observation("w1.7", 100), observation("w1.7.0", 200)],
            "hostPID": 100, "root": "w1.7", "panelID": 30, "alive": [100, 200, 201]}
        payload.update(change)
        result = subprocess.run([str(self.binary)], input=json.dumps(payload),
                                capture_output=True, check=True, text=True, timeout=3)
        return json.loads(result.stdout)

    def test_valid_services_are_unique_sorted_and_may_own_the_root(self):
        samples = [observation("w1.7", 100), observation("w1.7.0", 201),
                   observation("w1.7.1", 200), observation("w1.7.2", 200)]
        self.assertEqual(self.request(samples), {"status": "bound", "receiver_pids": [100, 200, 201]})
        self.assertEqual(self.request([observation("w1.7", 200), observation("w1.7.0", 200)]),
                         {"status": "bound", "receiver_pids": [200]})

    def test_unlinked_or_foreign_window_service_descendants_are_rejected_not_dropped(self):
        for native in (None, 0, 31):
            with self.subTest(native=native):
                result = self.request([observation("w1.7", 100), observation("w1.7.0", 200, native)])
                self.assertEqual(result["status"], "rejected", result)

    def test_invalid_or_dead_service_descendants_are_rejected_not_dropped(self):
        for pid, alive in ((0, [100]), (-1, [100]), (200, [100])):
            with self.subTest(pid=pid, alive=alive):
                result = self.request([observation("w1.7", 100), observation("w1.7.0", pid)], alive=alive)
                self.assertEqual(result["status"], "rejected", result)

    def test_host_context_may_have_another_or_unknown_native_window(self):
        samples = [observation("w1.7", 100), observation("w1.7.0", 100, None),
                   observation("w1.7.1", 100, 20), observation("w1.7.2", 200)]
        self.assertEqual(self.request(samples), {"status": "bound", "receiver_pids": [100, 200]})

    def test_outside_sheet_subtree_cannot_add_receivers_or_block_owned_binding(self):
        samples = [observation("w1.7", 100), observation("w1.7.0", 200),
                   observation("w1.70.0", 201, None), observation("w1.6", 0, 31),
                   observation("w2.0", 999, None)]
        self.assertEqual(self.request(samples), {"status": "bound", "receiver_pids": [100, 200]})

    def test_malformed_scope_and_subtree_references_are_rejected(self):
        for change in ({"hostPID": 0}, {"panelID": 0}, {"root": "not-a-ref"}):
            with self.subTest(change=change):
                self.assertEqual(self.request(**change)["status"], "rejected")
        for malformed in ("w1.7..0", "w1.7.x", "w1.7." + ".".join(["0"] * 13)):
            with self.subTest(ref=malformed):
                result = self.request([observation("w1.7", 100), observation(malformed, 200)])
                self.assertEqual(result["status"], "rejected", result)

    def test_selected_root_must_be_present_unique_alive_and_directly_panel_bound(self):
        variants = [[observation("w1.7.0", 200)],
                    [observation("w1.7", 100), observation("w1.7", 200)],
                    [observation("w1.7", 100, None), observation("w1.7.0", 200)],
                    [observation("w1.7", 100, 31), observation("w1.7.0", 200)]]
        for samples in variants:
            with self.subTest(samples=samples):
                self.assertEqual(self.request(samples)["status"], "rejected")
        self.assertEqual(self.request(alive=[200])["status"], "rejected")


if __name__ == "__main__":
    unittest.main()
