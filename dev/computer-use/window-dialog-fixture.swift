import AppKit
import Darwin

// Synthetic documents only. NSDocument still owns the ordinary save-panel and
// safe-save implementation; the URL guard limits its final destination.
final class OwnedDialogDocument: NSDocument, NSTextViewDelegate {
    let ownedFile: URL
    let allowsSave: Bool
    let windowFrame: NSRect
    let axIdentifier = "owned-document-" + UUID().uuidString
    private(set) var ownedWindow: NSWindow?
    private(set) var editor: NSTextView?
    private(set) var savePanel: NSSavePanel?
    private(set) var panelAXIdentifier = ""
    private(set) var initialText = ""
    private(set) var reads = 0
    private(set) var saveRequests = 0
    private(set) var dataRequests = 0
    private(set) var rejectedRequests = 0
    private(set) var lastError = ""

    override class var readableTypes: [String] { ["public.plain-text"] }
    override class var writableTypes: [String] { ["public.plain-text"] }

    init(ownedFile: URL, allowsSave: Bool, windowFrame: NSRect) {
        self.ownedFile = ownedFile
        self.allowsSave = allowsSave
        self.windowFrame = windowFrame
        super.init()
    }

    private func refusal(_ reason: String) -> NSError {
        rejectedRequests += 1
        lastError = reason
        return NSError(domain: "dev.cueward.WindowDialog", code: 1,
                       userInfo: [NSLocalizedDescriptionKey: reason])
    }

    private func requireOwnedURL(_ url: URL, writing: Bool) throws {
        guard url.isFileURL, url.standardizedFileURL == ownedFile,
              url.resolvingSymlinksInPath() == ownedFile else {
            throw refusal("Refused a destination outside the document's exact owned file")
        }
        guard !writing || allowsSave else {
            throw refusal("The bystander original is read-only in this fixture")
        }
        var status = stat()
        guard lstat(url.path, &status) == 0, (status.st_mode & S_IFMT) == S_IFREG,
              status.st_nlink == 1 else {
            throw refusal("Owned files must remain existing unlinked regular files")
        }
    }

    override func read(from url: URL, ofType typeName: String) throws {
        try MainActor.assumeIsolated {
            try requireOwnedURL(url, writing: false)
            do {
                try super.read(from: url, ofType: typeName)
                lastError = ""
            } catch {
                lastError = String(describing: error)
                throw error
            }
        }
    }

    override func read(from data: Data, ofType typeName: String) throws {
        try MainActor.assumeIsolated {
            guard let text = String(data: data, encoding: .utf8) else {
                throw CocoaError(.fileReadInapplicableStringEncoding)
            }
            reads += 1
            initialText = text
            editor?.string = text
        }
    }

    override func data(ofType typeName: String) throws -> Data {
        dataRequests += 1
        return Data((editor?.string ?? initialText).utf8)
    }

    override func writeSafely(to url: URL, ofType typeName: String,
                              for saveOperation: NSDocument.SaveOperationType) throws {
        // NSDocument's default canAsynchronouslyWrite is false. Enforce that
        // synchronous AppKit contract instead of weakening actor isolation.
        try MainActor.assumeIsolated {
            try requireOwnedURL(url, writing: true)
            do {
                // NSDocument may use its own internal temporary file, but the
                // final URL was checked before it can modify any destination.
                try super.writeSafely(to: url, ofType: typeName, for: saveOperation)
                lastError = ""
            } catch {
                lastError = String(describing: error)
                throw error
            }
        }
    }

    override func save(_ sender: Any?) {
        saveRequests += 1
        guard allowsSave else {
            _ = refusal("The bystander original cannot be saved")
            return
        }
        super.save(sender)
    }

    override func saveAs(_ sender: Any?) {
        saveRequests += 1
        guard allowsSave else {
            _ = refusal("The bystander original cannot be saved")
            return
        }
        super.saveAs(sender)
    }

    override func prepareSavePanel(_ panel: NSSavePanel) -> Bool {
        // Only identity is added for independent AX/native-window binding.
        // No filename, enabled state, first responder or input behavior changes.
        panelAXIdentifier = "owned-save-panel-" + UUID().uuidString
        panel.setAccessibilityIdentifier(panelAXIdentifier)
        savePanel = panel
        return super.prepareSavePanel(panel)
    }

    func openForSetup() throws {
        fileType = "public.plain-text"
        try read(from: ownedFile, ofType: "public.plain-text")
        fileURL = ownedFile
        updateChangeCount(.changeCleared)
    }

    func textDidChange(_ notification: Notification) { updateChangeCount(.changeDone) }

    override func makeWindowControllers() {
        guard ownedWindow == nil else { return }
        let window = NSWindow(contentRect: windowFrame,
                              styleMask: [.titled, .closable, .resizable],
                              backing: .buffered, defer: false)
        window.title = ownedFile.lastPathComponent
        window.isReleasedWhenClosed = false
        window.setAccessibilityIdentifier(axIdentifier)
        let content = NSView(frame: NSRect(origin: .zero, size: windowFrame.size))
        let scroll = NSScrollView(frame: NSRect(x: 20, y: 70,
            width: windowFrame.width - 40, height: windowFrame.height - 90))
        scroll.hasVerticalScroller = true
        scroll.autoresizingMask = [.width, .height]
        let editor = NSTextView(frame: scroll.bounds)
        editor.isRichText = false
        editor.delegate = self
        editor.string = initialText
        scroll.documentView = editor
        content.addSubview(scroll)
        if allowsSave {
            let button = NSButton(title: "Save As…", target: self, action: #selector(saveAs(_:)))
            button.frame = NSRect(x: 20, y: 20, width: 120, height: 32)
            content.addSubview(button)
        }
        window.contentView = content
        self.editor = editor
        ownedWindow = window
        addWindowController(NSWindowController(window: window))
        window.orderFront(nil)
    }

    var windowIdentity: [String: Any] {
        ["pid": getpid(), "window_id": ownedWindow?.windowNumber ?? 0,
         "file": ownedFile.path, "ax_identifier": axIdentifier]
    }

    var observation: [String: Any] {
        windowIdentity.merging([
            "initial_contents": initialText, "contents": editor?.string ?? "",
            "read_requests": reads, "save_requests": saveRequests,
            "data_requests": dataRequests, "document_edited": isDocumentEdited,
            "rejected_requests": rejectedRequests, "error": lastError
        ]) { _, value in value }
    }

    var dialogObservation: [String: Any]? {
        guard let window = ownedWindow, let panel = savePanel,
              panel.isVisible, panel.windowNumber > 0 else { return nil }
        let mode: String
        if panel.sheetParent === window, window.attachedSheet === panel {
            mode = "sheet"
        } else if panel.sheetParent == nil {
            mode = "window"
        } else {
            // A panel attached to some other document is not this target's dialog.
            return nil
        }
        return ["kind": "save", "mode": mode,
                "owner_window": windowIdentity,
                "window_id": panel.windowNumber, "ax_identifier": panelAXIdentifier,
                "owner_binding": "NSDocument.prepareSavePanel"]
    }
}

@MainActor final class WindowDialogLoop {
    let app = NSApplication.shared
    let target: OwnedDialogDocument
    let bystander: OwnedDialogDocument
    let stateURL: URL
    let lifetime: Double
    var activations = 0
    var observationSequence = 0
    var dialogEvents: [[String: Any]] = []
    var previousDialog: [String: Any]?

    init(stateURL: URL, targetFile: URL, bystanderFile: URL, lifetime: Double) {
        self.stateURL = stateURL
        self.lifetime = lifetime
        target = OwnedDialogDocument(ownedFile: targetFile, allowsSave: true,
            windowFrame: NSRect(x: 120, y: 220, width: 520, height: 340))
        bystander = OwnedDialogDocument(ownedFile: bystanderFile, allowsSave: false,
            windowFrame: NSRect(x: 680, y: 220, width: 520, height: 340))
    }

    func observeDialog() -> [String: Any]? {
        let dialog = target.dialogObservation
        let oldID = previousDialog?["ax_identifier"] as? String
        let newID = dialog?["ax_identifier"] as? String
        let oldWindowID = previousDialog?["window_id"] as? Int
        let newWindowID = dialog?["window_id"] as? Int
        let oldMode = previousDialog?["mode"] as? String
        let newMode = dialog?["mode"] as? String
        if oldID != newID || oldWindowID != newWindowID || oldMode != newMode {
            if let previousDialog, dialog == nil {
                dialogEvents.append(previousDialog.merging([
                    "event": "closed", "observation_sequence": observationSequence
                ]) { _, value in value })
            }
            if let dialog {
                dialogEvents.append(dialog.merging([
                    "event": previousDialog == nil ? "opened" : "changed",
                    "observation_sequence": observationSequence
                ]) { _, value in value })
            }
        }
        previousDialog = dialog
        return dialog
    }

    func saveState() {
        observationSequence += 1
        let dialog = observeDialog()
        var state = target.observation
        state.merge([
            "pid": getpid(), "active": app.isActive, "activations": activations,
            "observation_sequence": observationSequence,
            "owned_windows": [target.windowIdentity, bystander.windowIdentity],
            "documents": [target.observation, bystander.observation],
            "dialog": dialog as Any? ?? NSNull(), "dialog_events": dialogEvents,
            "panel_window_id": dialog?["window_id"] as? Int ?? 0
        ]) { _, value in value }
        do {
            let data = try JSONSerialization.data(withJSONObject: state)
            try data.write(to: stateURL, options: .atomic)
        } catch {
            fputs("Window-dialog observation failed: \(error)\n", stderr)
            exit(3)
        }
    }

    func run() {
        let activationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification, object: app, queue: .main
        ) { [self] _ in MainActor.assumeIsolated { activations += 1 } }
        app.setActivationPolicy(.accessory)
        do {
            // Both reads are setup, not attributed to later Agent actions.
            for document in [target, bystander] {
                try document.openForSetup()
                NSDocumentController.shared.addDocument(document)
                document.makeWindowControllers()
            }
        } catch {
            fputs("Window-dialog setup failed: \(error)\n", stderr)
            exit(2)
        }
        let timer = startDocumentObserver { [self] _ in MainActor.assumeIsolated { saveState() } }
        // These are disposable files; expiry must not wait on an unsaved prompt.
        DispatchQueue.main.asyncAfter(deadline: .now() + lifetime) { exit(0) }
        saveState()
        app.run()
        timer.invalidate()
        NotificationCenter.default.removeObserver(activationObserver)
    }
}

@main struct WindowDialogMain {
    static func main() {
        let environment = ProcessInfo.processInfo.environment
        guard let statePath = environment["CUEWARD_STATE"],
              let targetPath = environment["CUEWARD_FILE"],
              let bystanderPath = environment["CUEWARD_BYSTANDER"],
              [statePath, targetPath, bystanderPath].allSatisfy({ ($0 as NSString).isAbsolutePath })
        else { exit(2) }
        let stateURL = URL(fileURLWithPath: statePath).standardizedFileURL
        let targetFile = URL(fileURLWithPath: targetPath).standardizedFileURL
        let bystanderFile = URL(fileURLWithPath: bystanderPath).standardizedFileURL
        guard Set([stateURL, targetFile, bystanderFile]).count == 3,
              stateURL.resolvingSymlinksInPath() == stateURL else { exit(2) }
        let suppliedLifetime = Double(environment["CUEWARD_LIFETIME"] ?? "900") ?? 900
        guard suppliedLifetime.isFinite else { exit(2) }
        WindowDialogLoop(stateURL: stateURL, targetFile: targetFile, bystanderFile: bystanderFile,
                         lifetime: min(max(suppliedLifetime, 1), 900)).run()
    }
}
