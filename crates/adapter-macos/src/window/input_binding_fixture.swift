// Run the production status path with replacements only at operating-system boundaries.
import AppKit
import Darwin

var scenario: String { ProcessInfo.processInfo.environment["INPUT_BINDING_SCENARIO"]! }
let fixtureWindow = AXUIElementCreateApplication(101)
func AXIsProcessTrusted() -> Bool { true }
func CGPreflightPostEventAccess() -> Bool { true }
func AXUIElementSetMessagingTimeout(_ element: AXUIElement, _ timeout: Float) -> AXError { .success }
struct ForegroundApp { let processIdentifier: pid_t }
final class NSWorkspace {
    static let shared = NSWorkspace()
    var frontmostApplication: ForegroundApp? { ForegroundApp(processIdentifier: 1) }
}

func AXUIElementGetPid(_ element: AXUIElement, _ output: UnsafeMutablePointer<pid_t>) -> AXError {
    if scenario == "owner-error" { return .failure }
    output.pointee = scenario == "wrong-owner" ? pid + 1 : pid
    return .success
}

func AXUIElementCopyAttributeValue(_ element: AXUIElement, _ name: CFString,
                                  _ output: UnsafeMutablePointer<CFTypeRef?>) -> AXError {
    switch name as String {
    case kAXFocusedWindowAttribute:
        if scenario == "missing-focused" { return .noValue }
        output.pointee = scenario == "malformed-focused" ? "invalid" as CFString : fixtureWindow
    case kAXTitleAttribute:
        if ["title-missing", "legacy-missing-title"].contains(scenario) { return .attributeUnsupported }
        output.pointee = (scenario == "title-different" ? "AX dialog description" : expectedTitle) as CFString
    case kAXPositionAttribute:
        var point = expectedBounds.origin
        if scenario == "wrong-bounds" { point.x += 100 }
        output.pointee = AXValueCreate(.cgPoint, &point)
    case kAXSizeAttribute:
        var size = expectedBounds.size
        output.pointee = AXValueCreate(.cgSize, &size)
    default: return .attributeUnsupported
    }
    return .success
}

func CGWindowListCopyWindowInfo(_ options: CGWindowListOption, _ relative: CGWindowID) -> CFArray? {
    var rows: [[String: Any]] = [[
        kCGWindowNumber as String: NSNumber(value: windowID),
        kCGWindowOwnerPID as String: NSNumber(value: pid),
        kCGWindowName as String: expectedTitle,
        kCGWindowBounds as String: expectedBounds.dictionaryRepresentation,
    ]]
    if scenario == "legacy-ambiguous", options.contains(.optionAll) {
        var other = rows[0]
        other[kCGWindowNumber as String] = NSNumber(value: windowID + 1)
        rows.append(other)
    }
    return rows as CFArray
}

let fixtureLookup: @convention(c) (AXUIElement, UnsafeMutablePointer<CGWindowID>) -> AXError = { _, output in
    if scenario == "lookup-error" { return .cannotComplete }
    let observedID = UInt32(CommandLine.arguments[2])!
    output.pointee = scenario == "wrong-id" ? observedID + 1 : scenario == "zero-id" ? 0 : observedID
    return .success
}
func dlsym(_ handle: UnsafeMutableRawPointer?, _ symbol: UnsafePointer<CChar>) -> UnsafeMutableRawPointer? {
    if String(cString: symbol) == "_AXUIElementGetWindow" {
        return scenario.hasPrefix("legacy") ? nil : unsafeBitCast(fixtureLookup, to: UnsafeMutableRawPointer.self)
    }
    // Status checks entry-point presence only; no pointer events are created or posted.
    if String(cString: symbol) == "CGEventSetWindowLocation" { return UnsafeMutableRawPointer(bitPattern: 1) }
    return Darwin.dlsym(handle, symbol)
}
