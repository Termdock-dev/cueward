import AppKit
import WebKit

let stateURL = URL(fileURLWithPath: CommandLine.arguments[1])
let htmlURL = URL(fileURLWithPath: CommandLine.arguments[2])
let lifetime = CommandLine.arguments.count > 3 ? min(max(Double(CommandLine.arguments[3]) ?? 180, 1), 900) : 180
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let window = NSWindow(contentRect: NSRect(x: 100, y: 160, width: 500, height: 340),
                      styleMask: [.titled, .closable], backing: .buffered, defer: false)
window.title = "Cueward ordinary WKWebView fixture"
window.isReleasedWhenClosed = false
var activations = 0
var observations = 0
var foregroundChanges = 0
var pointerChanges = 0
var previousFront = NSWorkspace.shared.frontmostApplication?.processIdentifier
let initialPointer = NSEvent.mouseLocation
var domState: [String: Any] = [:]
func save() {
    observations += 1
    let front = NSWorkspace.shared.frontmostApplication?.processIdentifier
    if front != previousFront { foregroundChanges += 1; previousFront = front }
    if NSEvent.mouseLocation != initialPointer { pointerChanges += 1 }
    let value: [String: Any] = [
        "window_id": window.windowNumber, "pid": getpid(), "active": app.isActive,
        "activations": activations, "observations": observations,
        "foreground_changes": foregroundChanges, "pointer_different_samples": pointerChanges,
        "content_top": window.frame.height - (window.contentView?.frame.height ?? 0),
        "dom": domState,
    ]
    if let data = try? JSONSerialization.data(withJSONObject: value) {
        try? data.write(to: stateURL, options: .atomic)
    }
}
final class Receiver: NSObject, WKScriptMessageHandler {
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        if let value = message.body as? [String: Any] { domState = value; save() }
    }
}
let receiver = Receiver()
let configuration = WKWebViewConfiguration()
configuration.userContentController.add(receiver, name: "state")
// Ordinary WKWebView: no acceptsFirstMouse, hit-testing, mouse handler or event override.
let web = WKWebView(frame: NSRect(x: 0, y: 0, width: 500, height: 340), configuration: configuration)
window.contentView = web
web.loadFileURL(htmlURL, allowingReadAccessTo: htmlURL.deletingLastPathComponent())
window.orderFront(nil)
let observer = NotificationCenter.default.addObserver(forName: NSApplication.didBecomeActiveNotification,
    object: app, queue: .main) { _ in activations += 1 }
Timer.scheduledTimer(withTimeInterval: 0.01, repeats: true) { _ in save() }
DispatchQueue.main.asyncAfter(deadline: .now() + lifetime) { app.terminate(nil) }
save()
app.run()
