import AppKit

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let window = NSWindow(contentRect: NSRect(x: 150, y: 150, width: 440, height: 200),
                      styleMask: [.titled, .closable], backing: .buffered, defer: false)
window.title = "Cueward Wait Name Fixture"
let content = NSView(frame: NSRect(x: 0, y: 0, width: 440, height: 200))
for index in 0...1 {
    let button = NSButton(title: String(repeating: "測", count: 512) + String(index), target: nil, action: nil)
    button.frame = NSRect(x: 20, y: 120 - index * 40, width: 200, height: 32)
    button.setAccessibilityIdentifier("long-name-\(index)")
    content.addSubview(button)
}
let field = NSTextField(frame: NSRect(x: 20, y: 20, width: 360, height: 28))
field.stringValue = String(repeating: "值", count: 512) + "tail"
field.setAccessibilityIdentifier("long-value")
content.addSubview(field)
window.contentView = content
window.orderBack(nil)
app.run()
