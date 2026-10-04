import AppKit
import Darwin

final class ExistingDocument: NSDocument, NSTextViewDelegate {
    var ownedFile: URL?
    var editor: NSTextView?
    var saveButton: NSButton?
    var initialText = ""
    var reads = 0
    var saveRequests = 0
    var dataRequests = 0
    var lastError = ""
    override class var readableTypes: [String] { ["public.plain-text"] }
    override class var writableTypes: [String] { ["public.plain-text"] }

    override func read(from data: Data, ofType typeName: String) throws {
        guard let text = String(data: data, encoding: .utf8) else {
            throw CocoaError(.fileReadInapplicableStringEncoding)
        }
        reads += 1
        initialText = text
        editor?.string = text
    }

    override func data(ofType typeName: String) throws -> Data {
        dataRequests += 1
        return Data((editor?.string ?? "").utf8)
    }

    @objc func openOwnedFile(_ sender: Any?) {
        guard let ownedFile else { return }
        do {
            try read(from: ownedFile, ofType: "public.plain-text")
            fileURL = ownedFile
            updateChangeCount(.changeCleared)
            saveButton?.isEnabled = true
            windowControllers.first?.window?.title = ownedFile.lastPathComponent
            lastError = ""
        } catch { lastError = String(describing: error) }
    }

    @objc func requestSave(_ sender: Any?) {
        guard let ownedFile, fileURL == ownedFile else { return }
        saveRequests += 1
        save(sender)
    }

    func textDidChange(_ notification: Notification) { updateChangeCount(.changeDone) }

    override func makeWindowControllers() {
        guard let ownedFile else { exit(2) }
        let window = NSWindow(contentRect: NSRect(x: 150, y: 150, width: 520, height: 320),
                              styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Owned document editor"
        window.isReleasedWhenClosed = false
        let content = NSView(frame: NSRect(x: 0, y: 0, width: 520, height: 320))
        let editor = NSTextView(frame: NSRect(x: 20, y: 80, width: 480, height: 220))
        editor.isRichText = false
        editor.delegate = self
        editor.setAccessibilityLabel("Document contents")
        let openButton = NSButton(title: "Open \(ownedFile.lastPathComponent)", target: self,
                                  action: #selector(openOwnedFile(_:)))
        openButton.frame = NSRect(x: 20, y: 20, width: 260, height: 32)
        let saveButton = NSButton(title: "Save", target: self, action: #selector(requestSave(_:)))
        saveButton.frame = NSRect(x: 300, y: 20, width: 100, height: 32)
        // Ordinary document validation, not a native Save-panel enabled override.
        saveButton.isEnabled = false
        content.addSubview(editor); content.addSubview(openButton); content.addSubview(saveButton)
        window.contentView = content
        self.editor = editor
        self.saveButton = saveButton
        addWindowController(NSWindowController(window: window))
        window.orderFront(nil)
    }
}

@MainActor final class ReceiverLoop {
    let app = NSApplication.shared
    let document = ExistingDocument()
    let stateURL: URL
    let lifetime: Double
    var activations = 0
    var observationSequence = 0

    init(stateURL: URL, ownedFile: URL, lifetime: Double) {
        self.stateURL = stateURL
        self.lifetime = lifetime
        document.ownedFile = ownedFile
    }

    func saveState() {
        observationSequence += 1
        let state: [String: Any] = ["pid": getpid(), "active": app.isActive, "activations": activations,
            "observation_sequence": observationSequence,
            "window_id": document.windowControllers.first?.window?.windowNumber ?? 0,
            "file": document.fileURL?.path ?? "", "initial_contents": document.initialText,
            "contents": document.editor?.string ?? "", "read_requests": document.reads,
            "save_requests": document.saveRequests, "data_requests": document.dataRequests,
            "document_edited": document.isDocumentEdited, "error": document.lastError,
            "panel_window_id": document.windowControllers.first?.window?.attachedSheet?.windowNumber ?? 0]
        if let data = try? JSONSerialization.data(withJSONObject: state) {
            try? data.write(to: stateURL, options: .atomic)
        }
    }

    func run() {
        app.setActivationPolicy(.accessory)
        document.fileType = "public.plain-text"
        NSDocumentController.shared.addDocument(document)
        document.makeWindowControllers()
        let activationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification, object: app, queue: .main
        ) { [self] _ in MainActor.assumeIsolated { activations += 1 } }
        let timer = startDocumentObserver { [self] _ in MainActor.assumeIsolated { saveState() } }
        // Only synthetic data is owned; deadline must not wait on an unsaved-document prompt.
        DispatchQueue.main.asyncAfter(deadline: .now() + lifetime) { exit(0) }
        saveState()
        app.run()
        timer.invalidate()
        NotificationCenter.default.removeObserver(activationObserver)
    }
}

@main struct ExistingDocumentMain {
    static func main() {
        let environment = ProcessInfo.processInfo.environment
        guard let statePath = environment["CUEWARD_STATE"], let filePath = environment["CUEWARD_FILE"] else {
            exit(2)
        }
        let lifetime = min(max(Double(environment["CUEWARD_LIFETIME"] ?? "900") ?? 900, 1), 900)
        ReceiverLoop(stateURL: URL(fileURLWithPath: statePath), ownedFile: URL(fileURLWithPath: filePath),
                     lifetime: lifetime).run()
    }
}
