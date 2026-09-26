import AppKit
import Foundation

// A disposable receiver. It uses ordinary AppKit controls and event dispatch.
let stateURL = URL(fileURLWithPath: CommandLine.arguments[1])
let mode = CommandLine.arguments.dropFirst(2).first
let interrupt = mode == "interrupt"
final class InputWindow: NSWindow {
    override func accessibilityTitle() -> String? {
        mode == "ax-title-missing" ? nil : super.accessibilityTitle()
    }
}
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
var windows: [NSWindow] = []
var editors: [NSTextView] = []
var scrollViews: [NSScrollView] = []
var events: [[String: Any]] = []
let menu = NSMenu()
let edit = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
let editMenu = NSMenu(title: "Edit")
let selectAll = editMenu.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
edit.submenu = editMenu
menu.addItem(edit)
app.mainMenu = menu

for index in 0..<2 {
    let frame = NSRect(x: 100 + index * 60, y: 120, width: 400, height: 280)
    let window = InputWindow(contentRect: frame, styleMask: [.titled, .closable], backing: .buffered, defer: false)
    window.title = "Cueward input fixture \(index)"
    window.isReleasedWhenClosed = false
    let scroll = NSScrollView(frame: NSRect(origin: .zero, size: frame.size))
    scroll.hasVerticalScroller = true
    let editor = NSTextView(frame: NSRect(x: 0, y: 0, width: 380, height: 1800))
    editor.isRichText = false
    editor.isAutomaticQuoteSubstitutionEnabled = false
    editor.isAutomaticDashSubstitutionEnabled = false
    editor.font = NSFont.systemFont(ofSize: 18)
    if index == 0 { editor.string = (1...100).map { "Disposable line \($0)" }.joined(separator: "\n") }
    scroll.documentView = editor
    window.contentView = scroll
    window.makeFirstResponder(editor)
    selectAll.target = editor
    windows.append(window)
    editors.append(editor)
    scrollViews.append(scroll)
    window.orderFront(nil)
}

let monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .scrollWheel]) { event in
    events.append([
        "type": event.type.rawValue, "window": event.windowNumber,
        "tag": event.cgEvent?.getIntegerValueField(.eventSourceUserData) ?? 0,
        "flags": event.modifierFlags.rawValue,
    ])
    return event
}
func save() {
    if interrupt && editors[1].string.count >= 10 { windows[1].title = "Changed input fixture" }
    let state: [String: Any] = [
        "windows": windows.map { $0.windowNumber }, "texts": editors.map { $0.string },
        "titles": windows.map { $0.title },
        "scroll_y": scrollViews.map { $0.contentView.bounds.origin.y },
        "events": events, "active": app.isActive,
    ]
    if let data = try? JSONSerialization.data(withJSONObject: state) { try? data.write(to: stateURL, options: .atomic) }
}
Timer.scheduledTimer(withTimeInterval: 0.01, repeats: true) { _ in save() }
DispatchQueue.main.asyncAfter(deadline: .now() + 90) { app.terminate(nil) }
save()
app.run()
