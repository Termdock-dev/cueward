func documentURL(_ request: [String: Any]) -> URL {
    guard let path = request["file"] as? String, path.hasPrefix("/"), !path.contains("\0") else {
        fail("provide an absolute local file path")
    }
    let url = canonical(URL(fileURLWithPath: path))
    guard (try? url.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true,
          FileManager.default.isReadableFile(atPath: url.path) else {
        fail("document must be a readable regular file")
    }
    return url
}

func lockOpenRecipient(_ app: NSRunningApplication?, _ request: [String: Any]) -> Int32? {
    guard let app else { return nil }
    guard let directory = request["lock_dir"] as? String else { fail("missing app input lock directory") }
    let path = URL(fileURLWithPath: directory).appendingPathComponent("input-\(app.processIdentifier).lock").path
    let descriptor = open(path, O_CREAT | O_RDWR | O_NOFOLLOW, mode_t(0o600))
    guard descriptor >= 0 else { fail("cannot open app input lock") }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
        close(descriptor)
        fail("another input action is running for this app; observe before retrying")
    }
    return descriptor
}

func openDocument(_ request: [String: Any]) {
    let file = documentURL(request)
    let url = resolve(request)
    let newInstance = request["new_instance"] as? Bool ?? false
    let previous = Set(NSWorkspace.shared.runningApplications.filter { !$0.isTerminated }.map { $0.processIdentifier })
    let hint = "use --new-instance to request another process"
    let recipient = newInstance ? nil : existing(url, ambiguityHint: hint)
    let descriptor = lockOpenRecipient(recipient, request)
    defer { if let descriptor { close(descriptor) } }
    if !newInstance, recipient?.processIdentifier != existing(url, ambiguityHint: hint)?.processIdentifier {
        fail("document recipient changed before submission")
    }
    guard let before = NSWorkspace.shared.frontmostApplication?.processIdentifier,
          before > 0, recipient?.processIdentifier != before else {
        fail("foreground recipient or unavailable foreground state; background open stopped")
    }
    guard let caller = request["caller_pid"] as? Int32, caller > 0, kill(caller, 0) == 0 else {
        fail("document open caller exited")
    }
    let completion = LaunchCompletion<(NSRunningApplication?, Bool)>(deadline: ProcessInfo.processInfo.systemUptime + 20)
    NSWorkspace.shared.open([file], withApplicationAt: url,
                            configuration: workspaceConfiguration(newInstance: newInstance)) { app, error in
        completion.complete((app, error == nil))
    }
    let app = awaitApplication(completion, at: url, operation: "document open")
    if newInstance, previous.contains(app.processIdentifier) {
        fail("new-instance request returned an existing process; delivery is uncertain")
    }
    if let recipient, recipient.processIdentifier != app.processIdentifier {
        fail("document recipient changed; delivery is uncertain")
    }
    var result = applicationResult(app, "sent_unverified", before)
    result["file"] = file.path
    emit(result)
}
