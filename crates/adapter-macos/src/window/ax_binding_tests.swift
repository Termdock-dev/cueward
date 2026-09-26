let fixtureWindow = AXUIElementCreateApplication(101)
let otherWindow = AXUIElementCreateApplication(102)
let fixtureMenu = AXUIElementCreateApplication(103)
let scenario = CommandLine.arguments[8]

func AXUIElementCopyAttributeValue(_ element: AXUIElement, _ name: CFString,
                                  _ output: UnsafeMutablePointer<CFTypeRef?>) -> AXError {
    let name = name as String
    if CFEqual(element, application) {
        switch name {
        case kAXWindowsAttribute:
            output.pointee = (scenario == "deduplicate" ? [fixtureWindow]
                : scenario == "ambiguous-ax" ? [fixtureWindow, otherWindow] : []) as CFArray
        case kAXMainWindowAttribute:
            if scenario == "missing" { return .noValue }
            output.pointee = scenario == "other-main" ? otherWindow : fixtureWindow
        case kAXFocusedWindowAttribute:
            if scenario == "missing" { return .noValue }
            output.pointee = scenario == "other-focused" ? otherWindow : fixtureWindow
        case kAXMenuBarAttribute: output.pointee = fixtureMenu
        default: return .attributeUnsupported
        }
    } else {
        switch name {
        case kAXTitleAttribute:
            output.pointee = (CFEqual(element, otherWindow) && ["other-main", "other-focused"].contains(scenario)
                ? "Other window" : expectedTitle) as CFString
        case kAXPositionAttribute:
            var point = expectedBounds.origin
            output.pointee = AXValueCreate(.cgPoint, &point)
        case kAXSizeAttribute:
            var size = expectedBounds.size
            output.pointee = AXValueCreate(.cgSize, &size)
        default: return .attributeUnsupported
        }
    }
    return .success
}

func testWindowBinding() {
    let row: [String: Any] = [
        kCGWindowNumber as String: NSNumber(value: windowID),
        kCGWindowOwnerPID as String: NSNumber(value: pid),
        kCGWindowName as String: expectedTitle,
        kCGWindowIsOnscreen as String: false,
        kCGWindowBounds as String: expectedBounds.dictionaryRepresentation,
    ]
    observedCatalog = [row]
    if scenario == "ambiguous-catalog" {
        var duplicate = row
        duplicate[kCGWindowNumber as String] = NSNumber(value: windowID + 1)
        observedCatalog.append(duplicate)
    }
    let result = bindInspectionRoot(["menu", "other-main", "other-focused"].contains(scenario) ? "menu" : "window")
    guard CFEqual(result, scenario == "menu" ? fixtureMenu : fixtureWindow) else { fail("wrong root") }
    print("passed")
}
