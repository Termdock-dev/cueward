// Only OS boundaries are replaced. Production root resolution, fingerprints,
// guard ordering, locks, and dispatch/readback control flow remain executable.
import Darwin

var scenario: String { ProcessInfo.processInfo.environment["APP_AX_SCENARIO"] ?? "valid" }
var hostPID: pid_t { Int32(ProcessInfo.processInfo.environment["APP_AX_HOST"]!)! }
var receiverPID: pid_t { Int32(ProcessInfo.processInfo.environment["APP_AX_RECEIVER"]!)! }
let fixtureRoot = AXUIElementCreateApplication(101)
let fixtureGroup = AXUIElementCreateApplication(102)
let fixtureField = AXUIElementCreateApplication(103)
let replacementField = AXUIElementCreateApplication(104)
let otherRoot = AXUIElementCreateApplication(105)
let fixtureMenu = AXUIElementCreateApplication(106)
let menuItem = AXUIElementCreateApplication(107)
var lockOpens = 0
var foregroundReads = 0
var assignedValue: String?

func trace(_ event: String) {
    let path = ProcessInfo.processInfo.environment["APP_AX_TRACE"]!
    guard let file = FileHandle(forWritingAtPath: path) else { fail("fixture trace unavailable") }
    defer { file.closeFile() }
    file.seekToEndOfFile()
    file.write(Data((event + "\n").utf8))
}

func open(_ path: String, _ flags: Int32, _ mode: mode_t) -> Int32 {
    lockOpens += 1
    trace("lock:" + URL(fileURLWithPath: path).lastPathComponent)
    return Darwin.open(path, flags, mode)
}

func proc_pidinfo(_ pid: Int32, _ flavor: Int32, _ arg: UInt64,
                  _ buffer: UnsafeMutableRawPointer?, _ size: Int32) -> Int32 {
    let result = Darwin.proc_pidinfo(pid, flavor, arg, buffer, size)
    if result == size, let buffer, flavor == PROC_PIDTBSDINFO {
        let changedHost = scenario == "post-lock-instance" && pid == hostPID && lockOpens > 0
        let changedReceiver = pid == receiverPID && (scenario == "receiver-instance"
            || (scenario == "post-lock-receiver" && lockOpens > 0))
        if changedHost || changedReceiver {
            buffer.assumingMemoryBound(to: proc_bsdinfo.self).pointee.pbi_start_tvsec += 1
        }
    }
    return result
}

func kill(_ pid: pid_t, _ signal: Int32) -> Int32 {
    if scenario == "caller-exited" && signal == 0 { return -1 }
    return Darwin.kill(pid, signal)
}

struct ForegroundApp { let processIdentifier: pid_t }
final class NSWorkspace {
    static let shared = NSWorkspace()
    var frontmostApplication: ForegroundApp? {
        foregroundReads += 1
        switch scenario {
        case "foreground-host": return ForegroundApp(processIdentifier: hostPID)
        case "foreground-receiver": return ForegroundApp(processIdentifier: receiverPID)
        case "foreground-missing": return nil
        case "foreground-zero": return ForegroundApp(processIdentifier: 0)
        case "foreground-changed" where foregroundReads > 1: return ForegroundApp(processIdentifier: hostPID)
        default: return ForegroundApp(processIdentifier: 1)
        }
    }
}

func AXIsProcessTrusted() -> Bool { true }
func AXUIElementSetMessagingTimeout(_ element: AXUIElement, _ timeout: Float) -> AXError { .success }
func CGSessionCopyCurrentDictionary() -> CFDictionary? {
    ["CGSSessionScreenIsLocked": scenario == "post-lock-screen" && lockOpens > 0] as CFDictionary
}
func isField(_ element: AXUIElement) -> Bool {
    CFEqual(element, fixtureField) || CFEqual(element, replacementField)
}
func AXUIElementGetPid(_ element: AXUIElement, _ output: UnsafeMutablePointer<pid_t>) -> AXError {
    output.pointee = isField(element) || CFEqual(element, menuItem) ? receiverPID : hostPID
    return .success
}
func AXUIElementCopyActionNames(_ element: AXUIElement, _ output: UnsafeMutablePointer<CFArray?>) -> AXError {
    let actionable = isField(element) || CFEqual(element, menuItem)
    output.pointee = (actionable && scenario != "unsupported-press" ? [kAXPressAction] : []) as CFArray
    return .success
}
func AXUIElementIsAttributeSettable(_ element: AXUIElement, _ name: CFString,
                                   _ output: UnsafeMutablePointer<DarwinBoolean>) -> AXError {
    output.pointee = DarwinBoolean(isField(element) && scenario != "readonly")
    return .success
}

func applicationAttribute(_ key: String) -> CFTypeRef? {
    switch key {
    case kAXWindowsAttribute:
        return (scenario == "post-lock-context" && lockOpens > 0 ? [fixtureRoot, otherRoot] : [fixtureRoot]) as CFArray
    case kAXMainWindowAttribute, kAXFocusedWindowAttribute: return fixtureRoot
    case kAXMenuBarAttribute: return fixtureMenu
    default: return nil
    }
}

func nodeAttribute(_ element: AXUIElement, _ key: String) -> CFTypeRef? {
    switch key {
    case kAXRoleAttribute:
        return (isField(element) ? kAXTextFieldRole : CFEqual(element, fixtureGroup) ? kAXGroupRole
            : CFEqual(element, fixtureMenu) ? kAXMenuBarRole
            : CFEqual(element, menuItem) ? kAXMenuItemRole : kAXWindowRole) as CFString
    case kAXTitleAttribute:
        let changed = CFEqual(element, fixtureGroup) && scenario == "post-lock-ancestor" && lockOpens > 0
        return (changed ? "Changed group" : "Fixture") as CFString
    case kAXEnabledAttribute: return NSNumber(value: scenario != "disabled")
    case kAXValueAttribute:
        guard isField(element) else { return nil }
        if scenario == "post-lock-node" && lockOpens > 0 { return "Changed text" as CFString }
        return (assignedValue ?? "Fixture text") as CFString
    case kAXChildrenAttribute:
        let field = scenario == "post-lock-element" && lockOpens > 0 ? replacementField : fixtureField
        return (CFEqual(element, fixtureRoot) ? [fixtureGroup] : CFEqual(element, fixtureGroup) ? [field]
            : CFEqual(element, fixtureMenu) ? [menuItem] : []) as CFArray
    default: return nil
    }
}

func AXUIElementCopyAttributeValue(_ element: AXUIElement, _ key: CFString,
                                  _ output: UnsafeMutablePointer<CFTypeRef?>) -> AXError {
    if isField(element), key as String == kAXDescriptionAttribute, scenario == "unavailable-description" {
        return .failure
    }
    output.pointee = CFEqual(element, AXUIElementCreateApplication(hostPID))
        ? applicationAttribute(key as String) : nodeAttribute(element, key as String)
    return output.pointee == nil ? .attributeUnsupported : .success
}
func AXUIElementPerformAction(_ element: AXUIElement, _ action: CFString) -> AXError {
    trace("dispatch:press")
    return scenario == "dispatch-error" ? .cannotComplete : .success
}
func AXUIElementSetAttributeValue(_ element: AXUIElement, _ key: CFString, _ value: CFTypeRef) -> AXError {
    trace("dispatch:set_value")
    if scenario != "unconfirmed" { assignedValue = value as? String }
    return scenario == "dispatch-error" ? .cannotComplete : .success
}
