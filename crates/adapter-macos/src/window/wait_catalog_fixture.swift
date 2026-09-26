import ApplicationServices
import Foundation

var catalogReads = 0
func AXIsProcessTrusted() -> Bool { true }
func CGPreflightScreenCaptureAccess() -> Bool { true }

// Execute the production polling loop against a catalog that changes on each read.
func CGWindowListCopyWindowInfo(_ options: CGWindowListOption, _ relativeToWindow: CGWindowID) -> CFArray? {
    catalogReads += 1
    let scenario = CommandLine.arguments[8]
    if scenario == "unavailable" { return nil }
    if scenario == "absent" || (scenario == "close" && catalogReads > 1) { return [] as CFArray }
    var row: [String: Any] = [
        kCGWindowNumber as String: NSNumber(value: windowID),
        kCGWindowOwnerPID as String: NSNumber(value: pid),
        kCGWindowName as String: expectedTitle,
        kCGWindowIsOnscreen as String: false,
        kCGWindowBounds as String: expectedBounds.dictionaryRepresentation,
    ]
    if scenario == "rename" { row[kCGWindowName as String] = "Changed" }
    if scenario == "owner" { row[kCGWindowOwnerPID as String] = NSNumber(value: pid + 1) }
    if scenario == "bounds" { row[kCGWindowBounds as String] = expectedBounds.offsetBy(dx: 1, dy: 0).dictionaryRepresentation }
    if scenario == "fractional" {
        row[kCGWindowBounds as String] = CGRect(x: -100.5, y: 200.75, width: 640.5, height: 480.25).dictionaryRepresentation
    }
    return [row] as CFArray
}
