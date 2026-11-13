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

// Preserve unavailable, absent and duplicate observations for callers that classify lifecycle.
func catalogWindowRows() -> [[String: Any]]? {
    func rows(_ options: CGWindowListOption, relative: CGWindowID) -> [[String: Any]]? {
        guard let windows = CGWindowListCopyWindowInfo(options, relative) as? [[String: Any]] else { return nil }
        return windows.filter { ($0[kCGWindowNumber as String] as? NSNumber)?.uint32Value == windowID }
    }
    // Remote panel content can be absent from a single-window lookup but present
    // in the same complete catalog that issued the target. Re-read, never reuse it.
    if let matches = rows([.optionIncludingWindow, .excludeDesktopElements], relative: windowID), !matches.isEmpty {
        return matches
    }
    return rows([.optionAll, .excludeDesktopElements], relative: kCGNullWindowID)
}

func catalogWindowMatches(allowOffscreen: Bool = false, exactBounds: Bool = false) -> Bool {
    guard let matches = catalogWindowRows(), matches.count == 1 else { return false }
    return catalogWindowMatches(matches[0], allowOffscreen: allowOffscreen, exactBounds: exactBounds)
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

// Optional native identity avoids relying on AX titles, which dialogs omit and browsers extend.
typealias GetAXWindow = @convention(c) (AXUIElement, UnsafeMutablePointer<CGWindowID>) -> AXError
var axWindowSymbol: UnsafeMutableRawPointer? { dlsym(UnsafeMutableRawPointer(bitPattern: -2), "_AXUIElementGetWindow") }

func matchingWindows(comparingBounds: Bool = true) -> [AXUIElement] {
    var candidates = elements(application, kAXWindowsAttribute)
    for name in [kAXMainWindowAttribute, kAXFocusedWindowAttribute] {
        if let raw = attribute(application, name), CFGetTypeID(raw) == AXUIElementGetTypeID() {
            let element = raw as! AXUIElement
            if !candidates.contains(where: { CFEqual($0, element) }) { candidates.append(element) }
        }
    }
    if let symbol = axWindowSymbol {
        let lookup = unsafeBitCast(symbol, to: GetAXWindow.self)
        let native = candidates.filter { element in
            var observedID: CGWindowID = 0
            return lookup(element, &observedID) == .success && observedID == windowID
        }
        if !native.isEmpty { return native }
    }
    return candidates.filter { element in
        guard text(attribute(element, kAXTitleAttribute)) == expectedTitle else { return false }
        guard comparingBounds else { return true }
        guard let frame = bounds(element) else { return false }
        return sameBounds(frame, expectedBounds)
    }
}

func bindWindow() -> AXUIElement {
    validateCatalogWindow(allowOffscreen: true)
    let matches = matchingWindows()
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
