import AppKit
import Foundation

final class FixtureController: NSObject {
    let statePath: String
    let status: NSTextField
    var count = 0

    init(statePath: String, status: NSTextField) {
        self.statePath = statePath
        self.status = status
        super.init()
        writeState()
    }

    @objc func increment(_ sender: Any?) {
        count += 1
        status.stringValue = "pressed \(count)"
        writeState()
    }

    private func writeState() {
        try? String(count).write(toFile: statePath, atomically: true, encoding: .utf8)
    }
}

guard CommandLine.arguments.count == 2 else {
    fputs("expected state file path\n", stderr)
    exit(1)
}

let application = NSApplication.shared
application.setActivationPolicy(.accessory)

let window = NSWindow(
    contentRect: NSRect(x: 150, y: 150, width: 420, height: 220),
    styleMask: [.titled, .closable],
    backing: .buffered,
    defer: false
)
window.title = "Cueward AX Fixture"

let content = NSView(frame: NSRect(x: 0, y: 0, width: 420, height: 220))
let status = NSTextField(frame: NSRect(x: 40, y: 130, width: 300, height: 28))
status.isEditable = false
status.isSelectable = false
status.stringValue = "ready"
content.addSubview(status)

let controller = FixtureController(statePath: CommandLine.arguments[1], status: status)
let button = NSButton(frame: NSRect(x: 40, y: 70, width: 140, height: 32))
button.title = "Increment"
button.target = controller
button.action = #selector(FixtureController.increment(_:))
content.addSubview(button)

window.contentView = content
window.orderFront(nil)
application.run()
