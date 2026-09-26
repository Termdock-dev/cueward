let input = FileHandle.standardInput.readDataToEndOfFile()
guard let request = try? JSONSerialization.jsonObject(with: input) as? [String: Any],
      let ref = request["ref"] as? String,
      let expectedFingerprint = request["fingerprint"] as? String,
      let issuedAt = request["issued_at"] as? Double,
      let action = request["action"] as? String else { fail("invalid action request") }

let parts = ref.split(separator: ".", omittingEmptySubsequences: false)
let path = parts.compactMap { Int($0) }
guard path.first == 0, path.count == parts.count, path.count <= 13,
      path.allSatisfy({ $0 >= 0 }) else { fail("invalid element ref") }

let window = bindWindow()
var element = window
var parentIdentity = ""
var identity = fingerprint(describeElement(element), parent: parentIdentity)
for index in path.dropFirst() {
    let children = elements(element, kAXChildrenAttribute)
    guard index < children.count else { fail("element disappeared; inspect the window again") }
    element = children[index]
    parentIdentity = identity
    identity = fingerprint(describeElement(element), parent: parentIdentity)
}
let node = describeElement(element)
guard identity == expectedFingerprint,
      fingerprint(node, parent: parentIdentity) == expectedFingerprint else {
    fail("element changed; inspect the window again")
}
guard node["enabled"] as? Bool != false else { fail("element is disabled") }
validateCatalogWindow()
let age = Date().timeIntervalSince1970 - issuedAt
guard age >= 0 && age <= 300 else { fail("target expired; inspect the window again") }

let frontmostBefore = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
var status = "sent_unverified"
switch action {
case "press":
    guard (node["actions"] as? [String] ?? []).contains(kAXPressAction) else {
        fail("element does not support AXPress")
    }
    let error = AXUIElementPerformAction(element, kAXPressAction as CFString)
    guard error == .success else { fail("AXPress failed (\(error.rawValue)); action may have been delivered; inspect before retrying") }
    Thread.sleep(forTimeInterval: 0.1)
case "set_value":
    guard node["settable_value"] as? Bool == true,
          [kAXTextFieldRole, kAXTextAreaRole].contains(node["role"] as? String ?? ""),
          let value = request["value"] as? String else { fail("element does not support text value assignment") }
    let error = AXUIElementSetAttributeValue(element, kAXValueAttribute as CFString, value as CFString)
    guard error == .success else { fail("setting AXValue failed (\(error.rawValue)); action may have been delivered; inspect before retrying") }
    for _ in 0..<10 {
        if text(attribute(element, kAXValueAttribute)) == value {
            status = "confirmed"
            break
        }
        Thread.sleep(forTimeInterval: 0.05)
    }
default:
    fail("unsupported window action")
}

let frontmostAfter = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
emit([
    "action": action, "window_id": windowID, "ref": ref, "status": status,
    "frontmost_pid_before": frontmostBefore, "frontmost_pid_after": frontmostAfter,
    "foreground_changed": frontmostBefore != frontmostAfter,
])
