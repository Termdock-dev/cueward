import Cocoa
import Darwin

final class Delegate: NSObject, NSApplicationDelegate {
    var reopenCount = 0
    var activeCount = 0
    var window: NSWindow?
    let stateURL = Bundle.main.bundleURL.deletingLastPathComponent().appendingPathComponent("state.json")
    func save() {
        let state: [String: Any] = ["pid": getpid(), "reopens": reopenCount, "activations": activeCount]
        try? JSONSerialization.data(withJSONObject: state).write(to: stateURL, options: .atomic)
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        let window = NSWindow(contentRect: NSRect(x: 90, y: 90, width: 400, height: 240),
                              styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Cueward Application Fixture"
        window.orderBack(nil)
        self.window = window
        save()
    }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
        reopenCount += 1
        save()
        return false
    }
    func applicationDidBecomeActive(_ notification: Notification) { activeCount += 1; save() }
}
alarm(60)
let app = NSApplication.shared
let delegate = Delegate()
app.setActivationPolicy(.regular)
app.delegate = delegate
app.run()
