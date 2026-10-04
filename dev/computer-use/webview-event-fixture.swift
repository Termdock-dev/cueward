import AppKit
import WebKit

// Ordinary baseline, pass-through observation, and an explicitly patched control.
let stateURL = URL(fileURLWithPath: CommandLine.arguments[1])
let htmlURL = URL(fileURLWithPath: CommandLine.arguments[2])
let variant = CommandLine.arguments[3]
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let window = NSWindow(contentRect: NSRect(x: 100, y: 160, width: 500, height: 340),
                      styleMask: [.titled, .closable], backing: .buffered, defer: false)
window.title = "Cueward #32 WebView event receiver"
window.isReleasedWhenClosed = false
var dom: [String: Any] = [:]
var nativeEvents: [[String: Any]] = []
var firstMouse: [[String: Any]] = []
var callbacks = ["down": 0, "drag": 0, "up": 0]
var activations = 0
final class ObservedWebView: WKWebView {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        let native = super.acceptsFirstMouse(for: event)
        firstMouse.append(["native_result": native, "returned_result": variant == "control" || native,
                           "uptime": ProcessInfo.processInfo.systemUptime])
        return variant == "control" || native
    }
    override func mouseDown(with event: NSEvent) { callbacks["down", default: 0] += 1; super.mouseDown(with: event) }
    override func mouseDragged(with event: NSEvent) { callbacks["drag", default: 0] += 1; super.mouseDragged(with: event) }
    override func mouseUp(with event: NSEvent) { callbacks["up", default: 0] += 1; super.mouseUp(with: event) }
}
final class Receiver: NSObject, WKScriptMessageHandler {
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        if let value = message.body as? [String: Any] { dom = value }
    }
}
let receiver = Receiver()
let configuration = WKWebViewConfiguration()
configuration.userContentController.add(receiver, name: "state")
let frame = NSRect(x: 0, y: 0, width: 500, height: 340)
let web = variant == "ordinary" ? WKWebView(frame: frame, configuration: configuration)
    : ObservedWebView(frame: frame, configuration: configuration)
window.contentView = web
web.loadFileURL(htmlURL, allowingReadAccessTo: htmlURL.deletingLastPathComponent())
window.orderFront(nil)
let activationObservation = NotificationCenter.default.addObserver(forName: NSApplication.didBecomeActiveNotification,
    object: app, queue: .main) { _ in activations += 1 }
let eventObservation = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp, .mouseMoved]) { event in
    if event.windowNumber == window.windowNumber && nativeEvents.count < 5000 {
        let hit = web.hitTest(web.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow)
        nativeEvents.append(["type": event.type.rawValue, "uptime": ProcessInfo.processInfo.systemUptime,
            "timestamp": event.timestamp, "cg_timestamp": event.cgEvent?.timestamp ?? 0,
            "cg_subtype": event.cgEvent?.getIntegerValueField(.mouseEventSubtype) ?? -1,
            "window_point": [event.locationInWindow.x, event.locationInWindow.y],
            "tag": event.cgEvent?.getIntegerValueField(.eventSourceUserData) ?? 0,
            "source_pid": event.cgEvent?.getIntegerValueField(.eventSourceUnixProcessID) ?? -1,
            "source_state": event.cgEvent?.getIntegerValueField(.eventSourceStateID) ?? -2,
            "app_pressed_buttons": NSEvent.pressedMouseButtons,
            "combined_left_down": CGEventSource.buttonState(.combinedSessionState, button: .left),
            "hid_left_down": CGEventSource.buttonState(.hidSystemState, button: .left),
            "hit_class": hit.map { String(describing: type(of: $0)) } ?? "none"])
    }
    return event
}
func save() {
    let value: [String: Any] = ["pid": getpid(), "window_id": window.windowNumber,
        "active": app.isActive, "activations": activations, "variant": variant,
        "key": window.isKeyWindow, "main": window.isMainWindow,
        "uptime": ProcessInfo.processInfo.systemUptime, "dom": dom,
        "content_top": window.frame.height - (window.contentView?.frame.height ?? 0),
        "native_events": nativeEvents, "native_log_full": nativeEvents.count >= 5000,
        "first_mouse": firstMouse, "callbacks": callbacks]
    if let data = try? JSONSerialization.data(withJSONObject: value) { try? data.write(to: stateURL, options: .atomic) }
}
let timer = Timer(timeInterval: 0.02, repeats: true) { _ in save() }
for mode in [RunLoop.Mode.default, .modalPanel, .eventTracking] { RunLoop.main.add(timer, forMode: mode) }
DispatchQueue.main.asyncAfter(deadline: .now() + 120) { app.terminate(nil) }
save()
app.run()
