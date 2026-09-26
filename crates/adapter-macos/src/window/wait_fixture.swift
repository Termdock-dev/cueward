import AppKit

final class Controller: NSObject {
    let window: NSWindow
    let status: NSTextField
    let progress: NSTextField
    let next: NSButton
    init(_ window: NSWindow, _ status: NSTextField, _ progress: NSTextField, _ next: NSButton) {
        self.window = window; self.status = status; self.progress = progress; self.next = next
    }
    @objc func start(_ sender: Any?) {
        Timer.scheduledTimer(withTimeInterval: 1.5, repeats: false) { [self] _ in
            status.stringValue = "ready"
            next.isEnabled = true
            progress.removeFromSuperview()
        }
    }
    @objc func rename(_ sender: Any?) { window.title = "Changed Wait Fixture" }
    @objc func close(_ sender: Any?) { window.close() }
}
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let window = NSWindow(contentRect: NSRect(x: 150, y: 150, width: 440, height: 280),
                      styleMask: [.titled, .closable], backing: .buffered, defer: false)
window.isReleasedWhenClosed = false
window.title = "Cueward Wait Fixture"
let content = NSView(frame: NSRect(x: 0, y: 0, width: 440, height: 280))
let status = NSTextField(frame: NSRect(x: 20, y: 210, width: 360, height: 28))
status.stringValue = "loading"
status.setAccessibilityLabel("Status")
status.setAccessibilityIdentifier("status")
content.addSubview(status)
let progress = NSTextField(labelWithString: "Working")
progress.frame = NSRect(x: 20, y: 160, width: 360, height: 28)
progress.setAccessibilityIdentifier("progress")
content.addSubview(progress)
let next = NSButton(title: "Continue", target: nil, action: nil)
next.frame = NSRect(x: 20, y: 100, width: 100, height: 32)
next.isEnabled = false
content.addSubview(next)
let controller = Controller(window, status, progress, next)
for (index, entry) in [("Start", #selector(Controller.start(_:))), ("Rename", #selector(Controller.rename(_:))), ("Close", #selector(Controller.close(_:)))].enumerated() {
    let button = NSButton(title: entry.0, target: controller, action: entry.1)
    button.frame = NSRect(x: 20 + index * 130, y: 30, width: 110, height: 32)
    content.addSubview(button)
}
window.contentView = content
window.orderBack(nil)
app.run()
