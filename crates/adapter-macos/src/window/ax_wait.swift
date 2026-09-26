func checkedAttribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    let status = AXUIElementCopyAttributeValue(element, name as CFString, &value)
    switch status {
    case .success: return value
    case .attributeUnsupported, .noValue: return nil
    default: fail("wait could not read \(name) (\(status.rawValue)); observe again")
    }
}

struct WaitScan {
    var matches: [[String: Any]] = []
    var complete = true
    var visited = 0
    let deadline: Double
    let selector: [String: Any]
    let condition: String

    mutating func walk(_ element: AXUIElement, ref: String, depth: Int) {
        guard visited < 500, ProcessInfo.processInfo.systemUptime < deadline else { complete = false; return }
        visited += 1
        guard let role = text(checkedAttribute(element, kAXRoleAttribute)) else { fail("wait element has no role") }
        let title = text(checkedAttribute(element, kAXTitleAttribute))
        let description = text(checkedAttribute(element, kAXDescriptionAttribute))
        let name = [title, description].compactMap { $0 }.first { !$0.isEmpty } ?? ""
        var node: [String: Any] = ["role": role, "ref": ref,
            "name": String(name.prefix(512))]
        if let id = text(checkedAttribute(element, kAXIdentifierAttribute)) { node["identifier"] = id }
        if selectorMatches(node, selector) {
            if condition == "enabled", let enabled = checkedAttribute(element, kAXEnabledAttribute) as? Bool {
                node["enabled"] = enabled
            }
            let secure = role == "AXSecureTextField" || text(checkedAttribute(element, kAXSubroleAttribute)) == "AXSecureTextField"
            if condition == "value-equals" && !secure, let value = text(checkedAttribute(element, kAXValueAttribute)) { node["value"] = value }
            matches.append(node)
        }
        let children = elements(element, kAXChildrenAttribute)
        if depth >= 12 { if !children.isEmpty { complete = false }; return }
        for (index, child) in children.enumerated() {
            walk(child, ref: "\(ref).\(index)", depth: depth + 1)
            if !complete && (visited >= 500 || ProcessInfo.processInfo.systemUptime >= deadline) { break }
        }
    }
}

func windowState() -> String? {
    guard CGPreflightScreenCaptureAccess() else { fail("Screen Recording permission is required") }
    guard let rows = CGWindowListCopyWindowInfo([.optionIncludingWindow, .excludeDesktopElements], windowID) as? [[String: Any]] else {
        fail("window catalog is unavailable; absence is unproven")
    }
    guard rows.contains(where: { ($0[kCGWindowNumber as String] as? NSNumber)?.uint32Value == windowID }) else { return "window_gone" }
    return catalogWindowMatches(allowOffscreen: true, exactBounds: true) ? nil : "window_changed"
}

let input = FileHandle.standardInput.readDataToEndOfFile()
guard let request = try? JSONSerialization.jsonObject(with: input) as? [String: Any],
      let options = request["options"] as? [String: Any],
      let condition = options["condition"] as? String,
      let timeout = options["timeout_ms"] as? Double,
      let interval = options["interval_ms"] as? Double,
      let caller = request["caller_pid"] as? Int32 else { fail("invalid wait request") }
let started = ProcessInfo.processInfo.systemUptime
let deadline = started + timeout / 1000
var polls = 0
var matches: [[String: Any]] = []
var complete = false
var status = "timed_out"
while ProcessInfo.processInfo.systemUptime < deadline {
    guard caller > 0 && kill(caller, 0) == 0 else { fail("wait caller exited") }
    polls += 1
    if let state = windowState() { status = state == "window_gone" && condition == "window-gone" ? "matched" : state; break }
    if condition != "window-gone" {
        guard let selector = options["selector"] as? [String: Any] else { fail("missing wait selector") }
        var scan = WaitScan(deadline: deadline, selector: selector, condition: condition)
        scan.walk(bindWindow(), ref: "0", depth: 0)
        matches = scan.matches
        complete = scan.complete
        if let state = windowState() { status = state; break }
        if ProcessInfo.processInfo.systemUptime >= deadline { break }
        let decision = evaluateWait(condition, matches, complete: complete, value: options["value"] as? String)
        if decision != "pending" { status = decision; break }
    }
    Thread.sleep(forTimeInterval: max(0, min(interval / 1000, deadline - ProcessInfo.processInfo.systemUptime)))
}
var result: [String: Any] = ["window_id": windowID, "condition": condition, "status": status,
    "polls": polls, "elapsed_ms": Int((ProcessInfo.processInfo.systemUptime - started) * 1000),
    "match_count": matches.count, "complete": complete, "matched_ref": NSNull()]
if status == "matched", let ref = matches.first?["ref"] { result["matched_ref"] = ref }
emit(result)
