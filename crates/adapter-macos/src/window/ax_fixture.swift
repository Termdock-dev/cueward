import AppKit
import Foundation

final class FixtureController: NSObject {
    let statePath: String
    let status: NSTextField
    let input: NSTextField
    var count = 0

    init(statePath: String, status: NSTextField, input: NSTextField) {
        self.statePath = statePath
        self.status = status
        self.input = input
        super.init()
        writeState()
    }

    @objc func increment(_ sender: Any?) {
        count += 1
        status.stringValue = "pressed \(count)"
        writeState()
    }

    @objc func noEffect(_ sender: Any?) {}

    func writeState() {
        if let data = try? JSONSerialization.data(withJSONObject: ["count": count, "text": input.stringValue]) {
            try? data.write(to: URL(fileURLWithPath: statePath), options: .atomic)
        }
    }
}

guard CommandLine.arguments.count == 2 else {
    fputs("expected state file path\n", stderr)
    exit(1)
}

let application = NSApplication.shared
application.setActivationPolicy(.accessory)

let window = NSWindow(
    contentRect: NSRect(x: 150, y: 150, width: 420, height: 280),
    styleMask: [.titled, .closable],
    backing: .buffered,
    defer: false
)
window.title = "Cueward AX Fixture"

let content = NSView(frame: NSRect(x: 0, y: 0, width: 420, height: 280))
content.setAccessibilityElement(true)
content.setAccessibilityRole(.group)
content.setAccessibilityLabel("Editor")
let input = NSTextField(frame: NSRect(x: 40, y: 220, width: 300, height: 28))
input.stringValue = "draft"
input.setAccessibilityLabel("Draft")
content.addSubview(input)

let secret = NSSecureTextField(frame: NSRect(x: 40, y: 180, width: 300, height: 28))
secret.stringValue = "fixture-secret"
secret.setAccessibilityLabel("Secret")
content.addSubview(secret)

let status = NSTextField(frame: NSRect(x: 40, y: 130, width: 300, height: 28))
status.isEditable = false
status.isSelectable = false
status.stringValue = "ready"
content.addSubview(status)

let controller = FixtureController(statePath: CommandLine.arguments[1], status: status, input: input)
let button = NSButton(frame: NSRect(x: 40, y: 70, width: 140, height: 32))
button.title = "Increment"
button.target = controller
button.action = #selector(FixtureController.increment(_:))
content.addSubview(button)

let noop = NSButton(frame: NSRect(x: 210, y: 70, width: 140, height: 32))
noop.title = "No effect"
noop.target = controller
noop.action = #selector(FixtureController.noEffect(_:))
content.addSubview(noop)

Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { _ in controller.writeState() }

window.contentView = content
window.orderFront(nil)
application.run()
