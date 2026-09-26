import AppKit
import Foundation

let stateURL = URL(fileURLWithPath: CommandLine.arguments[1])
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
var window: NSWindow?
var retainedPanel: NSOpenPanel?
var result = ""
var activations = 0
let field = NSTextField(frame: NSRect(x: 20, y: 120, width: 300, height: 30))
field.stringValue = "ready"
field.setAccessibilityIdentifier("fixture-text")
let secret = NSSecureTextField(frame: NSRect(x: 20, y: 80, width: 300, height: 30))
secret.stringValue = "fixture-secret"

final class Actions: NSObject {
    @objc func create(_ sender: Any?) {
        guard window == nil else { return }
        let created = NSWindow(contentRect: NSRect(x: 120, y: 160, width: 360, height: 220),
            styleMask: [.titled, .closable], backing: .buffered, defer: false)
        created.title = "Cueward App AX Fixture"
        created.isReleasedWhenClosed = false
        created.contentView?.addSubview(field)
        created.contentView?.addSubview(secret)
        let button = NSButton(title: "Open Fixture Panel", target: self, action: #selector(openPanel(_:)))
        button.frame = NSRect(x: 20, y: 30, width: 200, height: 30)
        created.contentView?.addSubview(button)
        created.makeFirstResponder(field)
        window = created
        created.orderFront(nil)
    }
    @objc func openPanel(_ sender: Any?) {
        guard let window, retainedPanel == nil else { return }
        let opened = NSOpenPanel()
        opened.directoryURL = stateURL.deletingLastPathComponent()
        opened.canChooseDirectories = false
        retainedPanel = opened
        opened.beginSheetModal(for: window) { response in
            result = response == .cancel ? "cancelled" : "accepted"
            retainedPanel = nil
        }
    }
}
let actions = Actions()
let menu = NSMenu()
let file = NSMenuItem(title: "Fixture", action: nil, keyEquivalent: "")
let submenu = NSMenu(title: "Fixture")
submenu.autoenablesItems = false
let create = NSMenuItem(title: "New Fixture", action: #selector(Actions.create(_:)), keyEquivalent: "")
create.target = actions
submenu.addItem(create)
let disabled = NSMenuItem(title: "Disabled Fixture", action: #selector(Actions.create(_:)), keyEquivalent: "")
disabled.target = actions
disabled.isEnabled = false
submenu.addItem(disabled)
file.submenu = submenu
menu.addItem(file)
app.mainMenu = menu
let observer = NotificationCenter.default.addObserver(forName: NSApplication.didBecomeActiveNotification,
    object: app, queue: .main) { _ in activations += 1 }
func saveState() {
    let state: [String: Any] = ["pid": getpid(), "window": window != nil, "text": field.stringValue,
        "result": result, "active": app.isActive, "activations": activations]
    if let data = try? JSONSerialization.data(withJSONObject: state) {
        try? data.write(to: stateURL, options: .atomic)
    }
}
Timer.scheduledTimer(withTimeInterval: 0.03, repeats: true) { _ in saveState() }
DispatchQueue.main.asyncAfter(deadline: .now() + 120) { app.terminate(nil) }
saveState()
app.run()
