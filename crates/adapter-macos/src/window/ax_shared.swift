import AppKit
import ApplicationServices
import CryptoKit
import Foundation

func fail(_ message: String, code: Int32 = 1) -> Never {
    fputs("\(message)\n", stderr)
    exit(code)
}

func text(_ value: CFTypeRef?) -> String? {
    if let string = value as? String { return string }
    if let number = value as? NSNumber { return number.stringValue }
    return nil
}

func elements(_ element: AXUIElement, _ name: String) -> [AXUIElement] {
    var value: CFTypeRef?
    let status = AXUIElementCopyAttributeValue(element, name as CFString, &value)
    do {
        return try checkedElements(status: status, value: value)
    } catch {
        fail("failed to enumerate \(name): \(error); inspect the window again")
    }
}

func bounds(_ element: AXUIElement,
            reader: (AXUIElement, String) -> CFTypeRef? = attribute) -> CGRect? {
    guard let positionValue = reader(element, kAXPositionAttribute),
          let sizeValue = reader(element, kAXSizeAttribute),
          CFGetTypeID(positionValue) == AXValueGetTypeID(),
          CFGetTypeID(sizeValue) == AXValueGetTypeID() else { return nil }
    var point = CGPoint.zero
    var dimensions = CGSize.zero
    guard AXValueGetValue(positionValue as! AXValue, .cgPoint, &point),
          AXValueGetValue(sizeValue as! AXValue, .cgSize, &dimensions) else { return nil }
    return CGRect(origin: point, size: dimensions)
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

func describeElement(_ element: AXUIElement,
                     reader: (AXUIElement, String) -> CFTypeRef? = attribute) -> [String: Any] {
    let role = text(reader(element, kAXRoleAttribute)) ?? "unknown"
    let subrole = text(reader(element, kAXSubroleAttribute))
    let title = text(reader(element, kAXTitleAttribute))
    let description = text(reader(element, kAXDescriptionAttribute))
    let name = [title, description].compactMap { $0 }.first { !$0.isEmpty } ?? ""
    let secure = role == "AXSecureTextField" || subrole == "AXSecureTextField"
    var node: [String: Any] = [
        "role": role, "name": name,
        "actions": secure ? [] : actionNames(element),
        "settable_value": !secure && valueIsSettable(element),
    ]
    if let subrole { node["subrole"] = subrole }
    if !secure, let value = text(reader(element, kAXValueAttribute)) { node["value"] = value }
    if let identifier = text(reader(element, kAXIdentifierAttribute)) { node["identifier"] = identifier }
    if let enabled = reader(element, kAXEnabledAttribute) as? Bool { node["enabled"] = enabled }
    if let frame = bounds(element, reader: reader) {
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
