import AppKit

let stateURL = URL(fileURLWithPath: CommandLine.arguments[1])
let destination = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true)
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
final class Draft: NSDocument {
    var editor: NSTextView?
    var saveRequests = 0
    override class var readableTypes: [String] { ["public.plain-text"] }
    override class var writableTypes: [String] { ["public.plain-text"] }
    override func data(ofType typeName: String) throws -> Data { Data((editor?.string ?? "").utf8) }
    override func prepareSavePanel(_ savePanel: NSSavePanel) -> Bool {
        // Only the disposable destination and default filename are prepared.
        // No enabled state, activation, Space or event acceptance behavior is changed.
        savePanel.directoryURL = destination
        savePanel.nameFieldStringValue = "artifact.txt"
        return super.prepareSavePanel(savePanel)
    }
    @objc func requestSave(_ sender: Any?) { saveRequests += 1; save(sender) }
    override func makeWindowControllers() {
        let window = NSWindow(contentRect: NSRect(x: 150, y: 150, width: 480, height: 300),
                              styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Cueward unsaved document fixture"
        window.isReleasedWhenClosed = false
        let content = NSView(frame: NSRect(x: 0, y: 0, width: 480, height: 300))
        let editor = NSTextView(frame: NSRect(x: 20, y: 70, width: 440, height: 200))
        editor.isRichText = false
        editor.setAccessibilityIdentifier("draft-editor")
        editor.setAccessibilityLabel("Draft contents")
        let button = NSButton(title: "Save", target: self, action: #selector(requestSave(_:)))
        button.frame = NSRect(x: 20, y: 20, width: 100, height: 32)
        button.setAccessibilityIdentifier("save-document")
        content.addSubview(editor); content.addSubview(button)
        window.contentView = content
        window.makeFirstResponder(editor)
        self.editor = editor
        addWindowController(NSWindowController(window: window))
        window.orderFront(nil)
    }
}
let document = Draft()
document.fileType = "public.plain-text"
NSDocumentController.shared.addDocument(document)
document.makeWindowControllers()
var activations = 0
var foregroundChanges = 0
var lastFront = NSWorkspace.shared.frontmostApplication?.processIdentifier
let observer = NotificationCenter.default.addObserver(forName: NSApplication.didBecomeActiveNotification,
    object: app, queue: .main) { _ in activations += 1 }
func saveState() {
    let front = NSWorkspace.shared.frontmostApplication?.processIdentifier
    if front != lastFront { foregroundChanges += 1; lastFront = front }
    let state: [String: Any] = ["pid": getpid(), "window_id": document.windowControllers[0].window?.windowNumber ?? 0,
        "active": app.isActive, "activations": activations, "foreground_changes": foregroundChanges,
        "save_requests": document.saveRequests, "contents": document.editor?.string ?? "",
        "file_created": FileManager.default.fileExists(atPath: destination.appendingPathComponent("artifact.txt").path)]
    if let data = try? JSONSerialization.data(withJSONObject: state) { try? data.write(to: stateURL, options: .atomic) }
}
let observationTimer = startDocumentObserver { _ in saveState() }
DispatchQueue.main.asyncAfter(deadline: .now() + 180) { app.terminate(nil) }
saveState()
app.run()
