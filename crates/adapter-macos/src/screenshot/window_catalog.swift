import AppKit
import CoreGraphics
import Foundation

let frontmostPid = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
let includeOffscreen = CommandLine.arguments.dropFirst().first == "all"
let options: CGWindowListOption = [includeOffscreen ? .optionAll : .optionOnScreenOnly, .excludeDesktopElements]
guard let raw = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
    fputs("failed to read window list\n", stderr)
    exit(1)
}

let payload: [[String: Any]] = raw.compactMap { item in
    guard let windowId = item[kCGWindowNumber as String] as? NSNumber else { return nil }
    let ownerPid = (item[kCGWindowOwnerPID as String] as? NSNumber)?.intValue ?? 0
    let layer = (item[kCGWindowLayer as String] as? NSNumber)?.intValue ?? -1
    let alpha = (item[kCGWindowAlpha as String] as? NSNumber)?.doubleValue ?? 1.0
    let isOnscreen = (item[kCGWindowIsOnscreen as String] as? NSNumber)?.boolValue ?? false
    let owner = (item[kCGWindowOwnerName as String] as? String) ?? ""
    let title = (item[kCGWindowName as String] as? String) ?? ""
    let boundsDict = (item[kCGWindowBounds as String] as? NSDictionary) ?? [:]
    let rect = CGRect(dictionaryRepresentation: boundsDict) ?? .zero

    return [
        "window_id": windowId.uint32Value,
        "app": owner,
        "title": title,
        "owner_pid": ownerPid,
        "layer": layer,
        "alpha": alpha,
        "is_onscreen": isOnscreen,
        "is_frontmost": ownerPid == frontmostPid,
        "bounds": [
            "x": Int(rect.origin.x),
            "y": Int(rect.origin.y),
            "width": Int(rect.size.width),
            "height": Int(rect.size.height),
        ],
    ]
}

let data = try JSONSerialization.data(withJSONObject: payload, options: [])
if let text = String(data: data, encoding: .utf8) {
    print(text)
}
