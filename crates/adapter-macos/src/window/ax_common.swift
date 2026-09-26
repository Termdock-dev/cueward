import AppKit
import ApplicationServices
import CryptoKit
import Foundation

// Bound time spent waiting for unresponsive applications. Never retry actions automatically.
alarm(30)

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

func sameBounds(_ left: CGRect, _ right: CGRect) -> Bool {
    let tolerance = 3.0
    return abs(left.minX - right.minX) <= tolerance
        && abs(left.minY - right.minY) <= tolerance
        && abs(left.width - right.width) <= tolerance
        && abs(left.height - right.height) <= tolerance
}

func catalogWindowMatches(allowOffscreen: Bool = false, exactBounds: Bool = false) -> Bool {
    let options: CGWindowListOption = [.optionIncludingWindow, .excludeDesktopElements]
    let windows = CGWindowListCopyWindowInfo(options, windowID) as? [[String: Any]] ?? []
    guard let window = windows.first(where: {
        ($0[kCGWindowNumber as String] as? NSNumber)?.uint32Value == windowID
    }) else { return false }
    return catalogWindowMatches(window, allowOffscreen: allowOffscreen, exactBounds: exactBounds)
}

func catalogWindowMatches(_ window: [String: Any], allowOffscreen: Bool = false, exactBounds: Bool = false) -> Bool {
    guard (window[kCGWindowNumber as String] as? NSNumber)?.uint32Value == windowID,
        (window[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid,
        window[kCGWindowName as String] as? String == expectedTitle,
        (allowOffscreen || window[kCGWindowIsOnscreen as String] as? Bool == true),
        let rawBounds = window[kCGWindowBounds as String] as? NSDictionary,
        let frame = CGRect(dictionaryRepresentation: rawBounds),
        (exactBounds ? catalogBounds(frame) == expectedBounds : sameBounds(frame, expectedBounds)) else { return false }
    return true
}

func catalogBounds(_ frame: CGRect) -> CGRect {
    // Match window_catalog.swift's Int conversion, including negative origins.
    return CGRect(x: frame.origin.x.rounded(.towardZero), y: frame.origin.y.rounded(.towardZero),
                  width: frame.width.rounded(.towardZero), height: frame.height.rounded(.towardZero))
}

func validateCatalogWindow(allowOffscreen: Bool = false, exactBounds: Bool = false) {
    guard catalogWindowMatches(allowOffscreen: allowOffscreen, exactBounds: exactBounds) else {
        fail("window identity changed; inspect the window again")
    }
}

func bindWindow() -> AXUIElement {
    validateCatalogWindow(allowOffscreen: true)
    var candidates = elements(application, kAXWindowsAttribute)
    for name in [kAXMainWindowAttribute, kAXFocusedWindowAttribute] {
        if let raw = attribute(application, name), CFGetTypeID(raw) == AXUIElementGetTypeID() {
            let element = raw as! AXUIElement
            if !candidates.contains(where: { CFEqual($0, element) }) { candidates.append(element) }
        }
    }
    let matches = candidates.filter { element in
        guard text(attribute(element, kAXTitleAttribute)) == expectedTitle,
              let frame = bounds(element) else { return false }
        return sameBounds(frame, expectedBounds)
    }
    guard matches.count == 1 else {
        fail("window binding is unproven: \(matches.count) AX windows match window \(windowID)")
    }
    // A focused/main fallback must not hide another catalog window with the same identity.
    let rows = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    let catalogMatches = rows.filter { row in
        guard (row[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid,
              row[kCGWindowName as String] as? String == expectedTitle,
              let raw = row[kCGWindowBounds as String] as? NSDictionary,
              let frame = CGRect(dictionaryRepresentation: raw) else { return false }
        return sameBounds(frame, expectedBounds)
    }
    guard catalogMatches.count == 1 else { fail("window binding is ambiguous in the system catalog") }
    return matches[0]
}

func bindInspectionRoot(_ surface: String) -> AXUIElement {
    let window = bindWindow()
    if surface == "window" { return window }
    guard surface == "menu",
          let main = attribute(application, kAXMainWindowAttribute),
          CFGetTypeID(main) == AXUIElementGetTypeID(), CFEqual(main, window) else {
        fail("menu context is not the verified main window; inspect again")
    }
    guard let focused = attribute(application, kAXFocusedWindowAttribute),
          CFGetTypeID(focused) == AXUIElementGetTypeID(), CFEqual(focused, window) else {
        fail("menu context is not the verified focused window; inspect the current dialog or window")
    }
    guard let raw = attribute(application, kAXMenuBarAttribute),
          CFGetTypeID(raw) == AXUIElementGetTypeID() else { fail("app menu bar is unavailable") }
    return raw as! AXUIElement
}
