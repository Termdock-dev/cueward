import ApplicationServices
import Foundation

func fail(_ message: String, code: Int32 = 1) -> Never {
    fputs("\(message)\n", stderr)
    exit(code)
}

guard CommandLine.arguments.count == 10,
      let pid = Int32(CommandLine.arguments[1]),
      let windowID = UInt32(CommandLine.arguments[2]),
      let x = Double(CommandLine.arguments[4]),
      let y = Double(CommandLine.arguments[5]),
      let width = Double(CommandLine.arguments[6]),
      let height = Double(CommandLine.arguments[7]),
      let limit = Int(CommandLine.arguments[8]),
      let maxDepth = Int(CommandLine.arguments[9]) else {
    fail("invalid window inspection arguments")
}

guard AXIsProcessTrusted() else {
    fail("Accessibility permission is required", code: 3)
}

let expectedTitle = CommandLine.arguments[3]
let expectedBounds = CGRect(x: x, y: y, width: width, height: height)
let application = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(application, 2)

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else {
        return nil
    }
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
          CFGetTypeID(sizeValue) == AXValueGetTypeID() else {
        return nil
    }
    let position = positionValue as! AXValue
    let size = sizeValue as! AXValue
    var point = CGPoint.zero
    var dimensions = CGSize.zero
    guard AXValueGetValue(position, .cgPoint, &point),
          AXValueGetValue(size, .cgSize, &dimensions) else {
        return nil
    }
    return CGRect(origin: point, size: dimensions)
}

func sameBounds(_ left: CGRect, _ right: CGRect) -> Bool {
    let tolerance = 3.0
    return abs(left.minX - right.minX) <= tolerance
        && abs(left.minY - right.minY) <= tolerance
        && abs(left.width - right.width) <= tolerance
        && abs(left.height - right.height) <= tolerance
}

let matches = elements(application, kAXWindowsAttribute).filter { element in
    guard text(attribute(element, kAXTitleAttribute)) == expectedTitle,
          let frame = bounds(element) else {
        return false
    }
    return sameBounds(frame, expectedBounds)
}

guard matches.count == 1 else {
    fail("window binding is unproven: \(matches.count) AX windows match window \(windowID)")
}

func actionNames(_ element: AXUIElement) -> [String] {
    var names: CFArray?
    guard AXUIElementCopyActionNames(element, &names) == .success else {
        return []
    }
    return names as? [String] ?? []
}

func valueIsSettable(_ element: AXUIElement) -> Bool {
    var settable: DarwinBoolean = false
    guard AXUIElementIsAttributeSettable(element, kAXValueAttribute as CFString, &settable) == .success else {
        return false
    }
    return settable.boolValue
}

var nodes: [[String: Any]] = []
var truncated = false

func walk(_ element: AXUIElement, ref: String, parent: String?, depth: Int) {
    if nodes.count >= limit {
        truncated = true
        return
    }

    let role = text(attribute(element, kAXRoleAttribute)) ?? "unknown"
    let title = text(attribute(element, kAXTitleAttribute))
    let description = text(attribute(element, kAXDescriptionAttribute))
    let name = title ?? description ?? ""
    let value = role == "AXSecureTextField" ? nil : text(attribute(element, kAXValueAttribute))
    var node: [String: Any] = [
        "ref": ref,
        "role": role,
        "name": String(name.prefix(512)),
        "actions": actionNames(element),
        "settable_value": valueIsSettable(element),
    ]
    if let parent { node["parent_ref"] = parent }
    if let value { node["value"] = String(value.prefix(512)) }
    nodes.append(node)

    let children = elements(element, kAXChildrenAttribute)
    if depth >= maxDepth {
        if !children.isEmpty { truncated = true }
        return
    }
    for (index, child) in children.enumerated() {
        walk(child, ref: "\(ref).\(index)", parent: ref, depth: depth + 1)
    }
}

walk(matches[0], ref: "0", parent: nil, depth: 0)

let result: [String: Any] = [
    "window_id": windowID,
    "owner_pid": pid,
    "nodes": nodes,
    "truncated": truncated,
]
let data = try JSONSerialization.data(withJSONObject: result, options: [])
guard let output = String(data: data, encoding: .utf8) else {
    fail("failed to encode accessibility snapshot")
}
print(output)
