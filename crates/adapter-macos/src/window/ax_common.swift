import AppKit
import ApplicationServices
import CryptoKit
import Foundation

// Bound time spent waiting for unresponsive applications. Never retry actions automatically.
alarm(30)

func fail(_ message: String, code: Int32 = 1) -> Never {
    fputs("\(message)\n", stderr)
    exit(code)
}

guard CommandLine.arguments.count >= 8,
      let pid = Int32(CommandLine.arguments[1]),
      let windowID = UInt32(CommandLine.arguments[2]),
      let x = Double(CommandLine.arguments[4]),
      let y = Double(CommandLine.arguments[5]),
      let width = Double(CommandLine.arguments[6]),
      let height = Double(CommandLine.arguments[7]) else {
    fail("invalid window arguments")
}

guard AXIsProcessTrusted() else { fail("Accessibility permission is required", code: 3) }

let expectedTitle = CommandLine.arguments[3]
let expectedBounds = CGRect(x: x, y: y, width: width, height: height)
let application = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(application, 2)

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}

func text(_ value: CFTypeRef?) -> String? {
    if let string = value as? String { return string }
    if let number = value as? NSNumber { return number.stringValue }
    return nil
}

func elements(_ element: AXUIElement, _ name: String) -> [AXUIElement] {
    attribute(element, name) as? [AXUIElement] ?? []
}

func bounds(_ element: AXUIElement) -> CGRect? {
    guard let positionValue = attribute(element, kAXPositionAttribute),
          let sizeValue = attribute(element, kAXSizeAttribute),
          CFGetTypeID(positionValue) == AXValueGetTypeID(),
          CFGetTypeID(sizeValue) == AXValueGetTypeID() else { return nil }
    var point = CGPoint.zero
    var dimensions = CGSize.zero
    guard AXValueGetValue(positionValue as! AXValue, .cgPoint, &point),
          AXValueGetValue(sizeValue as! AXValue, .cgSize, &dimensions) else { return nil }
    return CGRect(origin: point, size: dimensions)
}

func sameBounds(_ left: CGRect, _ right: CGRect) -> Bool {
    let tolerance = 3.0
    return abs(left.minX - right.minX) <= tolerance
        && abs(left.minY - right.minY) <= tolerance
        && abs(left.width - right.width) <= tolerance
        && abs(left.height - right.height) <= tolerance
}

func validateCatalogWindow() {
    let options: CGWindowListOption = [.optionIncludingWindow, .excludeDesktopElements]
    let windows = CGWindowListCopyWindowInfo(options, windowID) as? [[String: Any]] ?? []
    guard let window = windows.first(where: {
        ($0[kCGWindowNumber as String] as? NSNumber)?.uint32Value == windowID
    }), (window[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid,
        window[kCGWindowName as String] as? String == expectedTitle,
        window[kCGWindowIsOnscreen as String] as? Bool == true,
        let rawBounds = window[kCGWindowBounds as String] as? NSDictionary,
        let frame = CGRect(dictionaryRepresentation: rawBounds),
        sameBounds(frame, expectedBounds) else { fail("window identity changed; inspect the window again") }
}

func bindWindow() -> AXUIElement {
    validateCatalogWindow()
    let matches = elements(application, kAXWindowsAttribute).filter { element in
        guard text(attribute(element, kAXTitleAttribute)) == expectedTitle,
              let frame = bounds(element) else { return false }
        return sameBounds(frame, expectedBounds)
    }
    guard matches.count == 1 else {
        fail("window binding is unproven: \(matches.count) AX windows match window \(windowID)")
    }
    return matches[0]
}

func actionNames(_ element: AXUIElement) -> [String] {
    var names: CFArray?
    guard AXUIElementCopyActionNames(element, &names) == .success else { return [] }
    return (names as? [String] ?? []).sorted()
}

func valueIsSettable(_ element: AXUIElement) -> Bool {
    var settable: DarwinBoolean = false
    guard AXUIElementIsAttributeSettable(element, kAXValueAttribute as CFString, &settable) == .success else {
        return false
    }
    return settable.boolValue
}

func describeElement(_ element: AXUIElement) -> [String: Any] {
    let role = text(attribute(element, kAXRoleAttribute)) ?? "unknown"
    let subrole = text(attribute(element, kAXSubroleAttribute))
    let title = text(attribute(element, kAXTitleAttribute))
    let description = text(attribute(element, kAXDescriptionAttribute))
    let name = [title, description].compactMap { $0 }.first { !$0.isEmpty } ?? ""
    let secure = role == "AXSecureTextField" || subrole == "AXSecureTextField"
    var node: [String: Any] = [
        "role": role, "name": name,
        "actions": secure ? [] : actionNames(element),
        "settable_value": !secure && valueIsSettable(element),
    ]
    if let subrole { node["subrole"] = subrole }
    if !secure, let value = text(attribute(element, kAXValueAttribute)) { node["value"] = value }
    if let identifier = text(attribute(element, kAXIdentifierAttribute)) { node["identifier"] = identifier }
    if let enabled = attribute(element, kAXEnabledAttribute) as? Bool { node["enabled"] = enabled }
    if let frame = bounds(element) {
        node["bounds"] = ["x": frame.minX, "y": frame.minY, "width": frame.width, "height": frame.height]
    }
    return node
}

func fingerprint(_ node: [String: Any], parent: String) -> String {
    guard let data = try? JSONSerialization.data(withJSONObject: [parent, node], options: [.sortedKeys]) else {
        fail("failed to encode element identity")
    }
    return SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
}

func emit(_ result: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: result),
          let output = String(data: data, encoding: .utf8) else { fail("failed to encode AX result") }
    print(output)
}
