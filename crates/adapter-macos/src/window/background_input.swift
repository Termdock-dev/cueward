let data = FileHandle.standardInput.readDataToEndOfFile()
guard let request = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
      let action = request["action"] as? String,
      let issuedAt = request["issued_at"] as? Double,
      let callerPID = request["caller_pid"] as? Int32, callerPID > 0,
      let lockPath = request["lock_path"] as? String,
      ["type_text", "key", "scroll", "click", "drag", "status"].contains(action) else { fail("invalid background input request") }
guard CGPreflightPostEventAccess() else { fail("Accessibility input permission is required", code: 3) }

// The process that posts events owns the lock, even if its caller is killed.
let lockFD = open(lockPath, O_CREAT | O_RDWR | O_NOFOLLOW, mode_t(0o600))
guard lockFD >= 0 else { fail("cannot open input lock") }
let acquiredLock = flock(lockFD, LOCK_EX | LOCK_NB) == 0
let inputBusy = !acquiredLock && errno == EWOULDBLOCK
guard acquiredLock || (action == "status" && inputBusy) else {
    close(lockFD)
    fail("another input action is running for this app; observe before retrying")
}
defer { close(lockFD) }
// Status is an observation, not a reservation across subsequent AX queries.
if action == "status" && acquiredLock {
    guard flock(lockFD, LOCK_UN) == 0 else { fail("cannot release status probe lock") }
}

let keyboard = ["type_text", "key"].contains(action)
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

func readinessIssue(forKeyboard: Bool) -> String? {
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
    if forKeyboard && !focusedWindowMatches() {
        return "target is not the app's verified keyboard window; input was not redirected"
    }
    return nil
}

func maySend() -> Bool {
    if let issue = readinessIssue(forKeyboard: keyboard) {
        if eventsSent == 0 { fail(issue) }
        interruption = issue
        return false
    }
    return true
}

// Optional platform functionality. Its presence proves a route, not app acceptance.
typealias SetWindowLocation = @convention(c) (CGEvent, CGFloat, CGFloat) -> Void
let framework = dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_LAZY)
let windowLocationSymbol = framework.flatMap { dlsym($0, "CGEventSetWindowLocation") }
let routingUnavailable = "background window-coordinate routing is unavailable on this system"
if action == "status" {
    let busyReason = inputBusy ? "another input action is running for this app" : nil
    func routeStatus(_ issue: String?) -> [String: Any] {
        var result: [String: Any] = ["dispatch_ready": issue == nil]
        if let issue { result["reason"] = issue }
        return result
    }
    emit([
        "window_id": windowID,
        "input_busy": inputBusy,
        "keyboard": routeStatus(busyReason ?? readinessIssue(forKeyboard: true)),
        "pointer": routeStatus(busyReason ?? readinessIssue(forKeyboard: false) ?? (windowLocationSymbol == nil ? routingUnavailable : nil)),
        "application_acceptance": "unverified",
    ])
    exit(0)
}

guard let source = CGEventSource(stateID: .privateState) else { fail("cannot create private input source") }
func configure(_ event: CGEvent, flags: CGEventFlags = []) {
    event.flags = flags
    event.setIntegerValueField(.eventSourceUserData, value: 0x43554549)
    event.setIntegerValueField(.eventTargetUnixProcessID, value: Int64(pid))
}

func point(_ xName: String = "x", _ yName: String = "y") -> CGPoint {
    guard let localX = request[xName] as? Double, let localY = request[yName] as? Double,
          localX.isFinite, localY.isFinite, localX >= 0, localY >= 0,
          localX < width, localY < height else { fail("input point must be inside the window frame") }
    return CGPoint(x: localX, y: localY)
}

func routePointer(_ event: CGEvent, at point: CGPoint) {
    guard let symbol = windowLocationSymbol else { fail(routingUnavailable) }
    configure(event)
    event.location = CGPoint(x: x + point.x, y: y + point.y)
    unsafeBitCast(symbol, to: SetWindowLocation.self)(event, point.x, point.y)
    for (field, value): (UInt32, Int64) in [(51, Int64(windowID)), (58, 1), (7, 3)] {
        guard let key = CGEventField(rawValue: field) else { fail("window routing field unavailable") }
        event.setIntegerValueField(key, value: value)
    }
    event.setIntegerValueField(.mouseEventWindowUnderMousePointer, value: Int64(windowID))
    event.setIntegerValueField(.mouseEventWindowUnderMousePointerThatCanHandleThisEvent, value: Int64(windowID))
}

func mouseEvent(_ type: CGEventType, at point: CGPoint, button: CGMouseButton = .left, count: Int64 = 1) -> CGEvent {
    guard let event = CGEvent(mouseEventSource: source, mouseType: type,
                              mouseCursorPosition: .zero, mouseButton: button) else { fail("cannot create pointer event") }
    routePointer(event, at: point)
    event.setIntegerValueField(.mouseEventClickState, value: count)
    event.setDoubleValueField(.mouseEventPressure, value: type == .leftMouseUp || type == .rightMouseUp ? 0 : 1)
    return event
}

func click() {
    let location = point()
    guard let name = request["button"] as? String, ["left", "right"].contains(name),
          let count = request["count"] as? Int, (1...2).contains(count) else { fail("invalid click") }
    let right = name == "right"
    // Prepare every pair before starting; allocation failure must not leave a down event.
    let pairs = (1...count).map { ordinal in (
        mouseEvent(right ? .rightMouseDown : .leftMouseDown, at: location, button: right ? .right : .left, count: Int64(ordinal)),
        mouseEvent(right ? .rightMouseUp : .leftMouseUp, at: location, button: right ? .right : .left, count: Int64(ordinal))
    ) }
    for (down, up) in pairs {
        guard maySend() else { break }
        down.postToPid(pid)
        up.postToPid(pid)
        eventsSent += 2
        if count == 2 { Thread.sleep(forTimeInterval: 0.05) }
    }
}

func drag() {
    let start = point(), end = point("to_x", "to_y")
    guard let duration = request["duration_ms"] as? Int, (50...2000).contains(duration) else { fail("invalid drag duration") }
    let steps = max(2, duration / 20)
    let down = mouseEvent(.leftMouseDown, at: start)
    var release = mouseEvent(.leftMouseUp, at: start)
    let sequence = (1...steps).map { index -> (CGEvent, CGEvent) in
        let fraction = Double(index) / Double(steps)
        let location = CGPoint(x: start.x + (end.x - start.x) * fraction, y: start.y + (end.y - start.y) * fraction)
        return (mouseEvent(.leftMouseDragged, at: location), mouseEvent(.leftMouseUp, at: location))
    }
    guard maySend() else { return }
    down.postToPid(pid)
    eventsSent += 1
    // Always release the original receiver at the last delivered point after a detected interruption.
    defer { release.postToPid(pid); eventsSent += 1 }
    for (event, up) in sequence {
        Thread.sleep(forTimeInterval: Double(duration) / Double(steps) / 1000)
        guard maySend() else { break }
        event.postToPid(pid)
        eventsSent += 1
        release = up
    }
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
          value.unicodeScalars.allSatisfy({ $0.properties.generalCategory != .control }) else {
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
    let location = point()
    guard let dx = request["delta_x"] as? Int32, let dy = request["delta_y"] as? Int32,
          (-4096...4096).contains(dx), (-4096...4096).contains(dy), dx != 0 || dy != 0 else {
        fail("invalid scroll input")
    }
    guard let event = CGEvent(scrollWheelEvent2Source: source, units: .pixel, wheelCount: 2,
                              wheel1: dy, wheel2: dx, wheel3: 0) else { fail("cannot create scroll event") }
    routePointer(event, at: location)
    if maySend() {
        event.postToPid(pid)
        eventsSent += 1
    }
case "click": click()
case "drag": drag()
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
