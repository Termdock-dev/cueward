import AppKit

// Ordinary AppKit dispatch. No acceptsFirstMouse, hit-test, global-input or activation override.
let stateURL = URL(fileURLWithPath: CommandLine.arguments[1])
let mode = CommandLine.arguments[2]
let human = mode == "human"
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let window = NSWindow(contentRect: NSRect(x: human ? 320 : 100, y: 180, width: 720, height: 440),
                      styleMask: [.titled, .closable], backing: .buffered, defer: false)
window.title = human ? "Cueward #31 human input" : "Cueward #31 background input"
window.isReleasedWhenClosed = false
window.acceptsMouseMovedEvents = true
let root = NSView(frame: NSRect(x: 0, y: 0, width: 720, height: 440))
window.contentView = root
var events: [[String: Any]] = []
var clicks = 0, selectAlls = 0, activations = 0
var startedAt: Double?
var phase = "waiting"
var interruptionTriggered = false
var lastModifiers: UInt = 0
var canvasDowns = 0, canvasDrags = 0, canvasUps = 0, scrollCallbacks = 0
var lastCanvasPoint = NSPoint.zero
let status = NSTextField(labelWithString: human ? "切英文輸入。按開始後，重複打 aAaA；大寫用 Shift，同時移動滑鼠。" : "準備中的背景接收端，請勿點擊或輸入。")
status.frame = NSRect(x: 20, y: 392, width: 680, height: 30)
root.addSubview(status)

final class Editor: NSTextView {
    override func selectAll(_ sender: Any?) {
        selectAlls += 1
        super.selectAll(sender)
    }
}
final class Scroll: NSScrollView {
    override func scrollWheel(with event: NSEvent) {
        scrollCallbacks += 1
        super.scrollWheel(with: event)
    }
}
final class ScrollDocument: NSView {
    override var isFlipped: Bool { true }
}
let scroll = Scroll(frame: NSRect(x: 20, y: 20, width: human ? 680 : 360, height: human ? 320 : 150))
scroll.hasVerticalScroller = true
let editor = Editor(frame: NSRect(x: 0, y: 0, width: human ? 650 : 330, height: 1400))
editor.isRichText = false
editor.isAutomaticQuoteSubstitutionEnabled = false
editor.isAutomaticDashSubstitutionEnabled = false
editor.font = NSFont.systemFont(ofSize: 22)
editor.setAccessibilityIdentifier("overlap-editor")
if human {
    scroll.documentView = editor
} else {
    // Separate the short-text receiver from the scroll challenge. AppKit sizes
    // an empty scrollable NSTextView to its viewport, including after attachment.
    editor.frame = NSRect(x: 20, y: 190, width: 360, height: 110)
    root.addSubview(editor)
    let document = ScrollDocument(frame: NSRect(x: 0, y: 0, width: 360, height: 1400))
    let rows = NSTextField(labelWithString: (1...50).map { "Synthetic scroll row \($0)" }.joined(separator: "\n"))
    rows.frame = document.bounds
    document.addSubview(rows)
    scroll.documentView = document
}
root.addSubview(scroll)
window.makeFirstResponder(editor)

final class Canvas: NSView {
    var point = NSPoint(x: 70, y: 60)
    var dragging = false
    var releases = 0
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        NSColor.white.setFill(); bounds.fill()
        NSColor.systemBlue.setFill()
        NSRect(x: point.x - 20, y: point.y - 20, width: 40, height: 40).fill()
    }
    override func mouseDown(with event: NSEvent) {
        canvasDowns += 1
        let p = convert(event.locationInWindow, from: nil)
        lastCanvasPoint = p
        dragging = abs(p.x - point.x) <= 20 && abs(p.y - point.y) <= 20
    }
    override func mouseDragged(with event: NSEvent) {
        canvasDrags += 1
        if dragging { point = convert(event.locationInWindow, from: nil); needsDisplay = true }
    }
    override func mouseUp(with event: NSEvent) { canvasUps += 1; dragging = false; releases += 1 }
}
let canvas = Canvas(frame: NSRect(x: 410, y: 160, width: 280, height: 180))
if !human { root.addSubview(canvas) }

final class Actions: NSObject {
    @objc func begin(_ sender: Any?) {
        guard startedAt == nil else { return }
        startedAt = ProcessInfo.processInfo.systemUptime
        phase = "running"
        editor.string = ""
        window.makeFirstResponder(editor)
    }
    @objc func countedClick(_ sender: Any?) { clicks += 1 }
    @objc func acknowledge(_ sender: Any?) { if phase == "finished" { phase = "acknowledged" } }
}
let actions = Actions()
let button = NSButton(title: human ? "開始 30 秒測試" : "Count background click", target: actions,
                      action: human ? #selector(Actions.begin(_:)) : #selector(Actions.countedClick(_:)))
button.frame = NSRect(x: 20, y: 350, width: 240, height: 32)
button.setAccessibilityIdentifier(human ? "begin-overlap" : "count-click")
root.addSubview(button)
let finish = NSButton(title: "完成並關閉", target: actions, action: #selector(Actions.acknowledge(_:)))
finish.frame = NSRect(x: 280, y: 350, width: 160, height: 32)
finish.isEnabled = false
if human { root.addSubview(finish) }
let menu = NSMenu()
let edit = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
let editMenu = NSMenu(title: "Edit")
let select = editMenu.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
select.target = editor
edit.submenu = editMenu
menu.addItem(edit)
app.mainMenu = menu
window.orderFront(nil)

let activeObservation = NotificationCenter.default.addObserver(forName: NSApplication.didBecomeActiveNotification,
    object: app, queue: .main) { _ in activations += 1 }
let eventObservation = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .flagsChanged,
    .leftMouseDown, .leftMouseUp, .leftMouseDragged, .mouseMoved, .scrollWheel]) { event in
    // Only this process's newly owned window is recorded. Never monitor global keyboard contents.
    if event.windowNumber == window.windowNumber && events.count < 20000 {
        // Independent identity-change stimulus, not an input-dispatch override.
        // It can exercise the sender's guard even when AppKit rejects first mouse.
        if mode == "interrupt" && event.type == .leftMouseDown && !interruptionTriggered
            && event.cgEvent?.getIntegerValueField(.eventSourceUserData) == 0x43554549 {
            interruptionTriggered = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.12) {
                window.title = "Cueward #31 changed receiver"
            }
        }
        if event.type == .flagsChanged { lastModifiers = event.modifierFlags.rawValue }
        var record: [String: Any] = ["uptime": ProcessInfo.processInfo.systemUptime,
            "type": event.type.rawValue, "flags": event.modifierFlags.rawValue,
            "tag": event.cgEvent?.getIntegerValueField(.eventSourceUserData) ?? 0,
            "source_pid": event.cgEvent?.getIntegerValueField(.eventSourceUnixProcessID) ?? -1,
            "source_state": event.cgEvent?.getIntegerValueField(.eventSourceStateID) ?? -2,
            "pointer": [event.cgEvent?.location.x ?? -1, event.cgEvent?.location.y ?? -1],
            "window_point": [event.locationInWindow.x, event.locationInWindow.y], "source": "owned_window"]
        if event.type == .keyDown || event.type == .keyUp {
            record["key_code"] = event.keyCode
            record["characters"] = event.characters ?? ""
            record["is_repeat"] = event.isARepeat
        }
        if event.type == .scrollWheel { record["scroll_delta_y"] = event.scrollingDeltaY }
        events.append(record)
    }
    return event
}
// Mouse positions only, never foreign keyboard/text/window contents. The user
// can move outside the test window while its keyboard focus remains unchanged.
let globalMouseObservation = human ? NSEvent.addGlobalMonitorForEvents(matching: .mouseMoved) { event in
    if events.count < 20000 {
        events.append(["type": event.type.rawValue, "uptime": ProcessInfo.processInfo.systemUptime,
            "flags": event.modifierFlags.rawValue, "tag": event.cgEvent?.getIntegerValueField(.eventSourceUserData) ?? 0,
            "source_pid": event.cgEvent?.getIntegerValueField(.eventSourceUnixProcessID) ?? -1,
            "source_state": event.cgEvent?.getIntegerValueField(.eventSourceStateID) ?? -2,
            "pointer": [event.cgEvent?.location.x ?? -1, event.cgEvent?.location.y ?? -1], "source": "global_mouse"])
    }
} : nil
func region(_ view: NSView) -> [String: Double] {
    ["x": view.frame.minX, "y": window.frame.height - view.frame.maxY,
     "width": view.frame.width, "height": view.frame.height]
}
func save() {
    if let startedAt, human {
        let remaining = max(0, 30 - Int(ProcessInfo.processInfo.systemUptime - startedAt))
        status.stringValue = remaining > 0 ? "剩 \(remaining) 秒：持續打 aAaA（Shift 大寫），移動滑鼠；勿切換 App。" : "測試結束，請放開 Shift，再按『完成並關閉』。"
        if remaining == 0 && phase == "running" { phase = "finished"; finish.isEnabled = true }
    }
    let value: [String: Any] = ["pid": getpid(), "window_id": window.windowNumber,
        "active": app.isActive, "activations": activations, "uptime": ProcessInfo.processInfo.systemUptime,
        "phase": phase, "started_at": startedAt as Any? ?? NSNull(), "text": editor.string,
        "selection_length": editor.selectedRange().length, "select_alls": selectAlls, "clicks": clicks,
        "scroll_y": scroll.contentView.bounds.origin.y,
        "object": ["x": canvas.point.x, "y": canvas.point.y], "dragging": canvas.dragging,
        "releases": canvas.releases, "interruption_triggered": interruptionTriggered,
        "canvas_callbacks": ["down": canvasDowns, "drag": canvasDrags, "up": canvasUps],
        "last_canvas_down": [lastCanvasPoint.x, lastCanvasPoint.y],
        "scroll_callbacks": scrollCallbacks, "document_height": scroll.documentView?.frame.height ?? 0,
        "scroll_viewport_height": scroll.contentView.bounds.height,
        "physical_modifiers": lastModifiers, "global_mouse_monitor": globalMouseObservation != nil,
        "regions": ["button": region(button), "canvas": region(canvas), "scroll": region(scroll)],
        "events": events, "event_log_full": events.count >= 20000]
    if let data = try? JSONSerialization.data(withJSONObject: value) { try? data.write(to: stateURL, options: .atomic) }
}
let timer = Timer(timeInterval: 0.02, repeats: true) { _ in save() }
for mode in [RunLoop.Mode.default, .modalPanel, .eventTracking] { RunLoop.main.add(timer, forMode: mode) }
DispatchQueue.main.asyncAfter(deadline: .now() + 600) { app.terminate(nil) }
save()
app.run()
