let data = FileHandle.standardInput.readDataToEndOfFile()
guard let request = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
      let action = request["action"] as? String,
      let issuedAt = request["issued_at"] as? Double,
      let callerPID = request["caller_pid"] as? Int32, callerPID > 0,
      let lockPath = request["lock_path"] as? String,
      ["type_text", "key", "scroll"].contains(action) else { fail("invalid background input request") }
guard CGPreflightPostEventAccess() else { fail("Accessibility input permission is required", code: 3) }

// The process that posts events owns the lock, even if its caller is killed.
let lockFD = open(lockPath, O_CREAT | O_RDWR | O_NOFOLLOW, mode_t(0o600))
guard lockFD >= 0 else { fail("cannot open input lock") }
guard flock(lockFD, LOCK_EX | LOCK_NB) == 0 else {
    close(lockFD)
    fail("another input action is running for this app; observe before retrying")
}
defer { close(lockFD) }

let keyboard = action != "scroll"
let startedAt = ProcessInfo.processInfo.systemUptime
let frontBefore = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
var eventsSent = 0
var interruption: String?

func focusedWindowMatches() -> Bool {
    // Inactive Spaces may omit AXWindows while still exposing AXFocusedWindow.
    guard let raw = attribute(application, kAXFocusedWindowAttribute),
          CFGetTypeID(raw) == AXUIElementGetTypeID() else { return false }
    let focused = raw as! AXUIElement
    guard text(attribute(focused, kAXTitleAttribute)) == expectedTitle,
          let frame = bounds(focused), sameBounds(frame, expectedBounds) else { return false }
    let rows = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    let matches = rows.filter { row in
        guard (row[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid,
              row[kCGWindowName as String] as? String == expectedTitle,
              let rawBounds = row[kCGWindowBounds as String] as? NSDictionary,
              let candidate = CGRect(dictionaryRepresentation: rawBounds) else { return false }
        return sameBounds(candidate, frame)
    }
    return matches.count == 1 && (matches[0][kCGWindowNumber as String] as? NSNumber)?.uint32Value == windowID
}

func readinessIssue() -> String? {
    if kill(callerPID, 0) != 0 { return "input caller exited; background input was stopped" }
    let age = Date().timeIntervalSince1970 - issuedAt
    if age < 0 || age > 300 { return "input target expired; take a new snapshot" }
    if ProcessInfo.processInfo.systemUptime - startedAt > 20 { return "input time limit reached; inspect before retrying" }
    if NSWorkspace.shared.frontmostApplication?.processIdentifier == pid {
        return "target app is in the foreground; background input was stopped"
    }
    if !catalogWindowMatches(allowOffscreen: true, exactBounds: true) {
        return "window changed; take a new snapshot"
    }
    if keyboard && !focusedWindowMatches() {
        return "target is not the app's verified keyboard window; input was not redirected"
    }
    return nil
}

func maySend() -> Bool {
    if let issue = readinessIssue() {
        if eventsSent == 0 { fail(issue) }
        interruption = issue
        return false
    }
    return true
}

guard let source = CGEventSource(stateID: .privateState) else { fail("cannot create private input source") }
func configure(_ event: CGEvent, flags: CGEventFlags = []) {
    event.flags = flags
    event.setIntegerValueField(.eventSourceUserData, value: 0x43554549)
    event.setIntegerValueField(.eventTargetUnixProcessID, value: Int64(pid))
}

func keyPair(code: CGKeyCode, unicode: String = "", flags: CGEventFlags = []) -> Bool {
    // Prepare both events before sending either. No checks or AX calls separate down and up.
    guard let down = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: true),
          let up = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: false) else {
        if eventsSent == 0 { fail("cannot create keyboard events") }
        interruption = "keyboard event creation failed"
        return false
    }
    for event in [down, up] {
        configure(event, flags: flags)
        let units = Array(unicode.utf16)
        if !units.isEmpty { event.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units) }
    }
    guard maySend() else { return false }
    down.postToPid(pid)
    up.postToPid(pid)
    eventsSent += 2
    return true
}

switch action {
case "type_text":
    guard let value = request["text"] as? String, !value.isEmpty,
          value.utf16.count <= 1024,
          value.unicodeScalars.allSatisfy({ !CharacterSet.controlCharacters.contains($0) }) else {
        fail("invalid text input")
    }
    for character in value {
        if !keyPair(code: 0, unicode: String(character)) { break }
        Thread.sleep(forTimeInterval: 0.002)
    }
case "key":
    guard let code = request["key_code"] as? UInt16, let flags = request["flags"] as? UInt64,
          flags & ~UInt64(0x1E0000) == 0 else { fail("invalid key input") }
    _ = keyPair(code: code, flags: CGEventFlags(rawValue: flags))
case "scroll":
    guard let localX = request["x"] as? Double, let localY = request["y"] as? Double,
          localX.isFinite, localY.isFinite, localX >= 0, localY >= 0,
          localX < width, localY < height,
          let dx = request["delta_x"] as? Int32, let dy = request["delta_y"] as? Int32,
          (-4096...4096).contains(dx), (-4096...4096).contains(dy), dx != 0 || dy != 0 else {
        fail("invalid scroll input")
    }
    // Window-local routing is optional platform functionality. Never fall back to global input.
    typealias SetWindowLocation = @convention(c) (CGEvent, CGFloat, CGFloat) -> Void
    guard let framework = dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_LAZY),
          let symbol = dlsym(framework, "CGEventSetWindowLocation") else {
        fail("background window-coordinate routing is unavailable on this system")
    }
    guard let event = CGEvent(scrollWheelEvent2Source: source, units: .pixel, wheelCount: 2,
                              wheel1: dy, wheel2: dx, wheel3: 0) else { fail("cannot create scroll event") }
    configure(event)
    event.location = CGPoint(x: x + localX, y: y + localY)
    unsafeBitCast(symbol, to: SetWindowLocation.self)(event, localX, localY)
    for (field, value): (UInt32, Int64) in [(51, Int64(windowID)), (58, 1), (7, 3)] {
        guard let key = CGEventField(rawValue: field) else { fail("window routing field unavailable") }
        event.setIntegerValueField(key, value: value)
    }
    event.setIntegerValueField(.mouseEventWindowUnderMousePointer, value: Int64(windowID))
    event.setIntegerValueField(.mouseEventWindowUnderMousePointerThatCanHandleThisEvent, value: Int64(windowID))
    if maySend() {
        event.postToPid(pid)
        eventsSent += 1
    }
default: fail("unsupported background input")
}

let frontAfter = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
var result: [String: Any] = [
    "action": action, "window_id": windowID,
    "status": interruption == nil ? "sent_unverified" : "partially_sent",
    "events_sent": eventsSent,
    "frontmost_pid_before": frontBefore, "frontmost_pid_after": frontAfter,
    "foreground_changed": frontBefore != frontAfter,
]
if let interruption { result["interruption"] = interruption }
emit(result)
