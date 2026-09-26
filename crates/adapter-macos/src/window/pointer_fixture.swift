import AppKit
import Foundation

let stateURL = URL(fileURLWithPath: CommandLine.arguments[1])
let interrupt = CommandLine.arguments.dropFirst(2).first == "interrupt"
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
var events: [[String: Any]] = []
var windows: [NSWindow] = []

final class Canvas: NSView {
    let acceptsBackground: Bool
    init(frame: NSRect, acceptsBackground: Bool) {
        self.acceptsBackground = acceptsBackground
        super.init(frame: frame)
    }
    required init?(coder: NSCoder) { fatalError("unused") }
    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { acceptsBackground }
    override func draw(_ dirtyRect: NSRect) {
        NSColor.gray.setFill()
        bounds.fill()
    }
    func record(_ event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        events.append([
            "type": event.type.rawValue, "window": event.windowNumber,
            "x": point.x, "y": point.y, "count": event.clickCount,
        ])
        if interrupt && event.type == .leftMouseDragged { window?.title = "Changed pointer fixture" }
    }
    override func mouseDown(with event: NSEvent) { record(event) }
    override func mouseUp(with event: NSEvent) { record(event) }
    override func rightMouseDown(with event: NSEvent) { record(event) }
    override func rightMouseUp(with event: NSEvent) { record(event) }
    override func mouseDragged(with event: NSEvent) { record(event) }
}

for index in 0..<2 {
    let frame = NSRect(x: 180 + index * 60, y: 160, width: 400, height: 280)
    let window = NSWindow(contentRect: frame, styleMask: [.titled, .closable], backing: .buffered, defer: false)
    window.title = "Cueward pointer fixture \(index)"
    window.isReleasedWhenClosed = false
    let canvas = Canvas(frame: NSRect(origin: .zero, size: frame.size), acceptsBackground: index == 0)
    window.contentView = canvas
    window.makeFirstResponder(canvas)
    windows.append(window)
    window.orderFront(nil)
}
func save() {
    let state: [String: Any] = [
        "windows": windows.map { $0.windowNumber }, "events": events,
        "active": app.isActive, "titles": windows.map { $0.title },
    ]
    if let data = try? JSONSerialization.data(withJSONObject: state) { try? data.write(to: stateURL, options: .atomic) }
}
Timer.scheduledTimer(withTimeInterval: 0.01, repeats: true) { _ in save() }
DispatchQueue.main.asyncAfter(deadline: .now() + 90) { app.terminate(nil) }
save()
app.run()
