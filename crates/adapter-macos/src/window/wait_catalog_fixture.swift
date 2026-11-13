import ApplicationServices
import Foundation

var catalogReads = 0
var fixtureUptime = 0.0
struct ProcessInfo {
    static let processInfo = ProcessInfo()
    var systemUptime: Double { fixtureUptime }
}
enum Thread {
    static func sleep(forTimeInterval interval: TimeInterval) { fixtureUptime += interval }
}
func AXIsProcessTrusted() -> Bool { true }
func CGPreflightScreenCaptureAccess() -> Bool { true }

// Execute the production polling loop against a catalog that changes on each read.
func CGWindowListCopyWindowInfo(_ options: CGWindowListOption, _ relativeToWindow: CGWindowID) -> CFArray? {
    catalogReads += 1
    let scenario = CommandLine.arguments[8]
    let individual = options.contains(.optionIncludingWindow)
    if scenario == "unavailable" { return nil }
    if scenario == "absent" || (scenario == "close" && catalogReads > 1) { return [] as CFArray }
    if scenario.hasPrefix("remote"), individual { return [] as CFArray }
    if scenario == "remote-unavailable" { return nil }
    var row: [String: Any] = [
        kCGWindowNumber as String: NSNumber(value: windowID),
        kCGWindowOwnerPID as String: NSNumber(value: pid),
        kCGWindowName as String: expectedTitle,
        kCGWindowIsOnscreen as String: false,
        kCGWindowBounds as String: expectedBounds.dictionaryRepresentation,
    ]
    if ["rename", "remote-rename"].contains(scenario) { row[kCGWindowName as String] = "Changed" }
    if ["owner", "remote-owner"].contains(scenario) || (scenario == "primary-conflict" && individual) {
        row[kCGWindowOwnerPID as String] = NSNumber(value: pid + 1)
    }
    if ["bounds", "remote-bounds"].contains(scenario) {
        row[kCGWindowBounds as String] = expectedBounds.offsetBy(dx: 1, dy: 0).dictionaryRepresentation
    }
    if scenario == "fractional" {
        row[kCGWindowBounds as String] = CGRect(x: -100.5, y: 200.75, width: 640.5, height: 480.25).dictionaryRepresentation
    }
    if ["duplicate", "remote-duplicate"].contains(scenario) { return [row, row] as CFArray }
    if scenario.hasPrefix("remote") {
        var bystander = row
        bystander[kCGWindowNumber as String] = NSNumber(value: windowID + 1)
        return [bystander, row] as CFArray
    }
    return [row] as CFArray
}
