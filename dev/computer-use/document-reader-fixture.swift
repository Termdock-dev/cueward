import AppKit
import Darwin

/// Shows one owned file. It has no document class, no editable view and no save path.
@MainActor final class ReaderLoop: NSObject {
    let app = NSApplication.shared
    let stateURL: URL
    let ownedFile: URL
    let lifetime: Double
    let window = NSWindow(contentRect: NSRect(x: 180, y: 180, width: 520, height: 320),
                          styleMask: [.titled, .closable], backing: .buffered, defer: false)
    let viewer = NSTextView(frame: NSRect(x: 20, y: 80, width: 480, height: 220))
    var openedFile = ""
    var content = ""
    var reads = 0
    var lastError = ""
    var activations = 0
    var observationSequence = 0

    init(stateURL: URL, ownedFile: URL, lifetime: Double) {
        self.stateURL = stateURL
        self.ownedFile = ownedFile
        self.lifetime = lifetime
    }

    // URL standardizing strips /private from /private/tmp, and the runner compares resolved paths.
    var resolvedPath: String {
        guard let resolved = realpath(ownedFile.path, nil) else { return ownedFile.path }
        defer { free(resolved) }
        return String(cString: resolved)
    }

    @objc func openOwnedFile(_ sender: Any?) {
        do {
            guard let text = String(data: try Data(contentsOf: ownedFile), encoding: .utf8) else {
                throw CocoaError(.fileReadInapplicableStringEncoding)
            }
            reads += 1
            content = text
            openedFile = resolvedPath
            viewer.string = text
            window.title = ownedFile.lastPathComponent
            lastError = ""
        } catch { lastError = String(describing: error) }
    }

    func saveState() {
        observationSequence += 1
        let state: [String: Any] = ["pid": getpid(), "active": app.isActive, "activations": activations,
            "observation_sequence": observationSequence, "window_id": window.windowNumber,
            "file": openedFile, "content": content, "read_requests": reads, "error": lastError]
        if let data = try? JSONSerialization.data(withJSONObject: state) {
            try? data.write(to: stateURL, options: .atomic)
        }
    }

    func showWindow() {
        window.title = "Owned document reader"
        window.isReleasedWhenClosed = false
        let content = NSView(frame: NSRect(x: 0, y: 0, width: 520, height: 320))
        viewer.isRichText = false
        viewer.isEditable = false
        viewer.setAccessibilityLabel("File contents")
        let openButton = NSButton(title: "Open \(ownedFile.lastPathComponent)", target: self,
                                  action: #selector(openOwnedFile(_:)))
        openButton.frame = NSRect(x: 20, y: 20, width: 260, height: 32)
        content.addSubview(viewer); content.addSubview(openButton)
        window.contentView = content
        window.orderFront(nil)
    }

    func run() {
        app.setActivationPolicy(.accessory)
        showWindow()
        let activationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification, object: app, queue: .main
        ) { [self] _ in MainActor.assumeIsolated { activations += 1 } }
        let timer = startDocumentObserver { [self] _ in MainActor.assumeIsolated { saveState() } }
        DispatchQueue.main.asyncAfter(deadline: .now() + lifetime) { exit(0) }
        saveState()
        app.run()
        timer.invalidate()
        NotificationCenter.default.removeObserver(activationObserver)
    }
}

@main struct DocumentReaderMain {
    static func main() {
        let environment = ProcessInfo.processInfo.environment
        guard let statePath = environment["CUEWARD_STATE"], let filePath = environment["CUEWARD_FILE"] else {
            exit(2)
        }
        let lifetime = min(max(Double(environment["CUEWARD_LIFETIME"] ?? "900") ?? 900, 1), 900)
        ReaderLoop(stateURL: URL(fileURLWithPath: statePath), ownedFile: URL(fileURLWithPath: filePath),
                   lifetime: lifetime).run()
    }
}
