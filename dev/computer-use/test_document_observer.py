"""Run the real fixture observer in default, modal-panel and event-tracking modes."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


class DocumentObserverTests(unittest.TestCase):
    def test_samples_continue_in_modal_and_tracking_modes(self):
        helper = Path(__file__).with_name("document-observer.swift").read_text()
        harness = '''
import AppKit
var currentMode = ""
var samples: [String: Int] = [:]
let timer = startDocumentObserver { _ in samples[currentMode, default: 0] += 1 }
for (name, mode) in [("default", RunLoop.Mode.default), ("modal_panel", .modalPanel), ("event_tracking", .eventTracking)] {
    currentMode = name
    samples[name] = 0
    let deadline = Date().addingTimeInterval(0.08)
    while Date() < deadline {
        if !RunLoop.main.run(mode: mode, before: deadline) { break }
    }
}
timer.invalidate()
print(String(data: try JSONSerialization.data(withJSONObject: samples), encoding: .utf8)!)
'''
        with tempfile.TemporaryDirectory(prefix="cueward-observer-test-") as directory:
            root = Path(directory)
            source, binary = root / "ObserverTest.swift", root / "ObserverTest"
            source.write_text(helper + "\n" + harness)
            subprocess.run(["swiftc", "-module-cache-path", str(root / "module-cache"), str(source), "-o", str(binary)],
                           check=True, capture_output=True, timeout=60)
            result = subprocess.run([str(binary)], check=True, capture_output=True, text=True, timeout=5)
        samples = json.loads(result.stdout)
        for mode in ("default", "modal_panel", "event_tracking"):
            with self.subTest(mode=mode):
                self.assertGreater(samples[mode], 0, samples)


if __name__ == "__main__":
    unittest.main()
