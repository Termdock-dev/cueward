import AppKit
import Darwin
import CoreGraphics

/// Synthetic visual state; no activation, first-mouse or input acceptance override.
@MainActor final class LockPatternView: NSView {
    var sequence = 1
    override func draw(_ dirtyRect: NSRect) {
        if let context = NSGraphicsContext.current?.cgContext {
            drawLockPattern(sequence: sequence, in: context)
        }
    }
}

@MainActor final class LockReceiver {
    let app = NSApplication.shared
    let stateURL: URL
    let runID: String
    let lifetime: Double
    let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 320, height: 224),
                          styleMask: [.titled, .closable], backing: .buffered, defer: false)
    let pattern = LockPatternView(frame: NSRect(x: 0, y: 64, width: 320, height: 160))
    let label = NSTextField(labelWithString: "SEQ:1")
    var sequence = 1
    var presses = 0
    var activations = 0
    var pressEvents: [[String: Any]] = []

    init(stateURL: URL, runID: String, lifetime: Double) {
        self.stateURL = stateURL; self.runID = runID; self.lifetime = lifetime
    }

    @objc func press(_ sender: Any?) {
        guard pressEvents.count < 6 else { exit(3) }
        presses += 1
        let session = CGSessionCopyCurrentDictionary() as? [String: Any]
        let locked: Any = session.map { ($0["CGSSessionScreenIsLocked"] as? Bool ?? false) as Any } ?? NSNull()
        pressEvents.append(["uptime": ProcessInfo.processInfo.systemUptime, "locked": locked])
        saveState()
    }

    func saveState() {
        let state: [String: Any] = ["run_id": runID, "pid": getpid(), "window_id": window.windowNumber,
            "sequence": sequence, "uptime": ProcessInfo.processInfo.systemUptime,
            "presses": presses, "press_events": pressEvents, "active": app.isActive, "activations": activations]
        do {
            try JSONSerialization.data(withJSONObject: state, options: [.sortedKeys])
                .write(to: stateURL, options: .atomic)
        } catch { exit(3) }
    }

    func advance() {
        sequence += 1
        pattern.sequence = sequence; pattern.needsDisplay = true
        label.stringValue = "SEQ:\(sequence)"
        pattern.displayIfNeeded()
        saveState()
    }

    func prepare() {
        app.setActivationPolicy(.accessory)
        window.title = "Owned lock-resource diagnostic"
        let content = NSView(frame: NSRect(x: 0, y: 0, width: 320, height: 224))
        let button = NSButton(title: "Record diagnostic action", target: self, action: #selector(press(_:)))
        button.frame = NSRect(x: 16, y: 16, width: 188, height: 30)
        button.setAccessibilityIdentifier("lock-action")
        label.frame = NSRect(x: 216, y: 16, width: 100, height: 30)
        label.setAccessibilityIdentifier("lock-sequence")
        content.addSubview(pattern); content.addSubview(button); content.addSubview(label)
        window.contentView = content
        window.orderFront(nil)
        pattern.displayIfNeeded()
        saveState()
    }

    func run() {
        prepare()
        let activation = NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification, object: app, queue: .main
        ) { [self] _ in MainActor.assumeIsolated { activations += 1; saveState() } }
        let timer = Timer(timeInterval: 0.5, repeats: true) { [self] _ in
            MainActor.assumeIsolated { advance() }
        }
        RunLoop.main.add(timer, forMode: .common)
        DispatchQueue.main.asyncAfter(deadline: .now() + lifetime) { exit(0) }
        app.run()
        timer.invalidate()
        NotificationCenter.default.removeObserver(activation)
    }
}

@main struct LockReceiverMain {
    static func main() {
        let env = ProcessInfo.processInfo.environment
        guard let path = env["CUEWARD_STATE"], let runID = env["CUEWARD_RUN_ID"],
              let lifetime = Double(env["CUEWARD_LIFETIME"] ?? "180"), (1...600).contains(lifetime) else { exit(2) }
        LockReceiver(stateURL: URL(fileURLWithPath: path), runID: runID, lifetime: lifetime).run()
    }
}
