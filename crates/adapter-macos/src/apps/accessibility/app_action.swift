guard let target = request["target"] as? [String: Any],
      let expectedApp = target["app"] as? [String: Any],
      let reference = target["ref"] as? String,
      let expectedRoot = target["root_fingerprint"] as? String,
      let expectedContext = target["context"] as? String,
      let expectedNode = target["fingerprint"] as? String,
      let issuedAt = target["issued_at"] as? Double,
      let action = request["action"] as? String,
      let caller = request["caller_pid"] as? Int32,
      let lockDirectory = request["lock_dir"] as? String else { fail("invalid app action request") }
guard fingerprint(expectedApp, parent: "") == fingerprint(instance, parent: "") else {
    fail("application instance changed; inspect again")
}
let roots = AppRoots()
guard roots.context == expectedContext else { fail("app context changed; inspect again") }
let (element, parentID, rootID) = resolve(reference, roots)
let node = appNode(element)
guard rootID == expectedRoot, fingerprint(node, parent: parentID) == expectedNode else {
    fail("app element changed; inspect again")
}
guard node["enabled"] as? Bool != false else { fail("app element is disabled") }
if reference.hasPrefix("menu") {
    guard node["role"] as? String == kAXMenuItemRole, elements(element, kAXChildrenAttribute).isEmpty else {
        fail("only leaf menu items can be pressed")
    }
}
let receiver = elementPID(element)
var locks: [Int32] = []
defer { for descriptor in locks { close(descriptor) } }
for owner in Set([pid, receiver]).sorted() {
    let path = URL(fileURLWithPath: lockDirectory).appendingPathComponent("input-\(owner).lock").path
    let descriptor = open(path, O_CREAT | O_RDWR | O_NOFOLLOW, mode_t(0o600))
    guard descriptor >= 0 else { fail("cannot open app input lock") }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
        close(descriptor)
        fail("another input action is running for this app; inspect before retrying")
    }
    locks.append(descriptor)
}
let age = Date().timeIntervalSince1970 - issuedAt
guard age >= 0 && age <= 300 else { fail("app target expired; inspect again") }
validateObservation(expectedContext)
let (fresh, freshParent, freshRoot) = resolve(reference, AppRoots())
guard CFEqual(fresh, element), freshRoot == expectedRoot,
      fingerprint(appNode(fresh), parent: freshParent) == expectedNode else {
    fail("app element changed before dispatch; inspect again")
}
guard caller > 0 && kill(caller, 0) == 0 else { fail("app action caller exited") }
guard let frontBefore = NSWorkspace.shared.frontmostApplication?.processIdentifier,
      frontBefore > 0, frontBefore != pid, frontBefore != receiver else {
    fail("target app or AX receiver is in the foreground; background action stopped")
}
var status = "sent_unverified"
switch action {
case "press":
    guard (node["actions"] as? [String] ?? []).contains(kAXPressAction) else { fail("element does not support AXPress") }
    let error = AXUIElementPerformAction(element, kAXPressAction as CFString)
    guard error == .success else { fail("AXPress failed (\(error.rawValue)); delivery is uncertain; inspect before retrying") }
    Thread.sleep(forTimeInterval: 0.1)
case "set_value":
    guard node["settable_value"] as? Bool == true,
          [kAXTextFieldRole, kAXTextAreaRole].contains(node["role"] as? String ?? ""),
          let value = request["value"] as? String else { fail("element does not support text value assignment") }
    let error = AXUIElementSetAttributeValue(element, kAXValueAttribute as CFString, value as CFString)
    guard error == .success else { fail("AXValue assignment failed (\(error.rawValue)); delivery is uncertain; inspect before retrying") }
    for _ in 0..<10 {
        if text(attribute(element, kAXValueAttribute)) == value { status = "confirmed"; break }
        Thread.sleep(forTimeInterval: 0.05)
    }
default: fail("unsupported app action")
}
let frontAfter = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
emit(["action": action, "pid": pid, "receiver_pid": receiver, "ref": reference, "status": status,
      "frontmost_pid_before": frontBefore, "frontmost_pid_after": frontAfter,
      "foreground_changed": frontBefore != frontAfter])
