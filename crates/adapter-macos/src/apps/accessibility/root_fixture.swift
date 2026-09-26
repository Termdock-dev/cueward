// Deterministic AX observations; no desktop access or input is required.
var scenario: String { ProcessInfo.processInfo.environment["APP_AX_SCENARIO"] ?? "fallback" }
func AXIsProcessTrusted() -> Bool { true }
func CGSessionCopyCurrentDictionary() -> CFDictionary? {
    ["CGSSessionScreenIsLocked": scenario == "locked"] as CFDictionary
}
let fixtureRoot = AXUIElementCreateApplication(101)
let duplicateRoot = AXUIElementCreateApplication(102)
let fixtureChild = AXUIElementCreateApplication(103)
let fixtureMenu = AXUIElementCreateApplication(104)
let menuItem = AXUIElementCreateApplication(105)
var windowReads = 0

func AXUIElementGetPid(_ element: AXUIElement, _ result: UnsafeMutablePointer<pid_t>) -> AXError {
    result.pointee = scenario == "service" && CFEqual(element, fixtureChild) ? getpid() : getppid()
    return .success
}
func AXUIElementCopyActionNames(_ element: AXUIElement, _ result: UnsafeMutablePointer<CFArray?>) -> AXError {
    result.pointee = (CFEqual(element, menuItem) ? [kAXPressAction] : []) as CFArray
    return .success
}
func AXUIElementIsAttributeSettable(_ element: AXUIElement, _ name: CFString,
                                   _ result: UnsafeMutablePointer<DarwinBoolean>) -> AXError {
    result.pointee = DarwinBoolean(CFEqual(element, fixtureChild))
    return .success
}
func AXUIElementCopyAttributeValue(_ element: AXUIElement, _ key: CFString,
                                  _ result: UnsafeMutablePointer<CFTypeRef?>) -> AXError {
    let key = key as String
    if CFEqual(element, application) {
        switch key {
        case kAXWindowsAttribute:
            windowReads += 1
            result.pointee = (scenario == "ambiguous" || (scenario == "changed" && windowReads > 1)
                ? [fixtureRoot, duplicateRoot] : scenario == "deduplicate" ? [fixtureRoot] : []) as CFArray
        case kAXMainWindowAttribute, kAXFocusedWindowAttribute:
            if scenario == "failed-read" { return .cannotComplete }
            if scenario == "windowless" { return .noValue }
            result.pointee = fixtureRoot
        case kAXMenuBarAttribute: result.pointee = fixtureMenu
        default: return .attributeUnsupported
        }
    } else {
        switch key {
        case kAXRoleAttribute:
            if scenario == "failed-role" { return .failure }
            result.pointee = (CFEqual(element, fixtureChild) ? kAXTextFieldRole
                : CFEqual(element, fixtureMenu) ? kAXMenuBarRole
                : CFEqual(element, menuItem) ? kAXMenuItemRole : kAXWindowRole) as CFString
        case kAXTitleAttribute: result.pointee = "Fixture" as CFString
        case kAXDescriptionAttribute:
            if scenario == "unavailable-description" { return .failure }
            return .attributeUnsupported
        case kAXValueAttribute:
            guard CFEqual(element, fixtureChild) else { return .noValue }
            if scenario == "unavailable-value" { return .failure }
            if scenario == "failed-value" { return .cannotComplete }
            result.pointee = "Fixture text" as CFString
        case kAXChildrenAttribute:
            result.pointee = (CFEqual(element, fixtureRoot) ? [fixtureChild]
                : CFEqual(element, fixtureMenu) ? [menuItem] : []) as CFArray
        default: return .attributeUnsupported
        }
    }
    return .success
}
