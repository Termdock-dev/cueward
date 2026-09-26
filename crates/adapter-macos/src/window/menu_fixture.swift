import AppKit
import Foundation

let stateURL = URL(fileURLWithPath: CommandLine.arguments[1])
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let window = NSWindow(contentRect: NSRect(x: 200, y: 180, width: 480, height: 320),
                      styleMask: [.titled, .closable], backing: .buffered, defer: false)
window.title = "Cueward Menu Fixture"
window.isReleasedWhenClosed = false
let field = NSTextField(frame: NSRect(x: 30, y: 160, width: 400, height: 30))
field.stringValue = "Disposable document"
window.contentView?.addSubview(field)
window.makeFirstResponder(field)
var count = 0
var result = ""
var retainedPanel: NSSavePanel?

final class Actions: NSObject {
    @objc func increment(_ sender: Any?) { count += 1 }
    @objc func save(_ sender: Any?) {
        let sheet = NSSavePanel()
        retainedPanel = sheet
        sheet.directoryURL = stateURL.deletingLastPathComponent()
        sheet.nameFieldStringValue = "Result.txt"
        sheet.beginSheetModal(for: window) { response in
            if response == .OK, let url = sheet.url {
                do {
                    try field.stringValue.write(to: url, atomically: true, encoding: .utf8)
                    result = "saved"
                } catch { result = "write failed" }
            } else { result = "cancelled" }
            retainedPanel = nil
        }
    }
}
let actions = Actions()
let menu = NSMenu()
let file = NSMenuItem(title: "File", action: nil, keyEquivalent: "")
let submenu = NSMenu(title: "File")
for (title, selector) in [("Increment", #selector(Actions.increment(_:))),
                          ("Save Document", #selector(Actions.save(_:)))] {
    let item = NSMenuItem(title: title, action: selector, keyEquivalent: "")
    item.target = actions
    submenu.addItem(item)
}
file.submenu = submenu
menu.addItem(file)
app.mainMenu = menu
window.orderFront(nil)
func saveState() {
    let state: [String: Any] = [
        "count": count, "active": app.isActive, "result": result,
        "sheet": window.attachedSheet?.windowNumber ?? 0,
    ]
    if let data = try? JSONSerialization.data(withJSONObject: state) {
        try? data.write(to: stateURL, options: .atomic)
    }
}
Timer.scheduledTimer(withTimeInterval: 0.03, repeats: true) { _ in saveState() }
DispatchQueue.main.asyncAfter(deadline: .now() + 90) { app.terminate(nil) }
saveState()
app.run()
