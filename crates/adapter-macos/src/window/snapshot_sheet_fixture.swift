import AppKit
import Foundation

guard CommandLine.arguments.count == 3 else { exit(1) }
let stateURL = URL(fileURLWithPath: CommandLine.arguments[1])
let attachPath = CommandLine.arguments[2]
let app = NSApplication.shared
app.setActivationPolicy(.accessory)

let parent = NSWindow(
    contentRect: NSRect(x: 180, y: 180, width: 320, height: 200),
    styleMask: [.titled, .closable], backing: .buffered, defer: false
)
parent.title = "Snapshot Sheet Fixture"
parent.backgroundColor = .systemBlue
let sheet = NSPanel(
    contentRect: NSRect(x: 0, y: 0, width: 640, height: 360),
    styleMask: [.titled], backing: .buffered, defer: false
)
sheet.title = "Attached sheet"
sheet.backgroundColor = .systemOrange
var attaching = false

func writeState(attached: Bool) {
    let state: [String: Any] = [
        "window_id": parent.windowNumber,
        "attached": attached,
        "extends_parent": !parent.frame.contains(sheet.frame),
    ]
    if let data = try? JSONSerialization.data(withJSONObject: state) {
        try? data.write(to: stateURL, options: .atomic)
    }
}

parent.orderFront(nil)
DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { writeState(attached: false) }
Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { timer in
    guard !attaching, FileManager.default.fileExists(atPath: attachPath) else { return }
    attaching = true
    parent.beginSheet(sheet)
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.7) {
        writeState(attached: sheet.sheetParent === parent)
    }
    timer.invalidate()
}
// Bound fixture lifetime even if the test runner exits unexpectedly.
DispatchQueue.main.asyncAfter(deadline: .now() + 40) { app.terminate(nil) }
app.run()
