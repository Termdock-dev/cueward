import ApplicationServices
import Foundation

// Supply catalog observations without depending on a display layout or AX permission.
var observedCatalog: [[String: Any]] = []
var individualCatalog: [[String: Any]]?
var catalogUnavailable = false
func CGWindowListCopyWindowInfo(_ options: CGWindowListOption, _ relativeToWindow: CGWindowID) -> CFArray? {
    if options.contains(.optionIncludingWindow) { return (individualCatalog ?? observedCatalog) as CFArray }
    guard relativeToWindow == kCGNullWindowID, !catalogUnavailable else { return nil }
    return observedCatalog as CFArray
}
func AXIsProcessTrusted() -> Bool { true }

func testCatalogBounds() {
    func observe(_ frame: CGRect) {
        observedCatalog = [[
            kCGWindowNumber as String: NSNumber(value: windowID),
            kCGWindowOwnerPID as String: NSNumber(value: pid),
            kCGWindowName as String: expectedTitle,
            kCGWindowIsOnscreen as String: false,
            kCGWindowBounds as String: frame.dictionaryRepresentation,
        ]]
    }
    func require(_ condition: Bool, _ message: String) {
        guard condition else { fail(message) }
    }

    observe(expectedBounds)
    require(catalogWindowMatches(allowOffscreen: true, exactBounds: true), "integer frame rejected")
    require(!catalogWindowMatches(exactBounds: true), "offscreen guard bypassed")

    // Catalog serialization truncates each coordinate toward zero, including negative origins.
    observe(CGRect(x: -100.5, y: 200.75, width: 640.5, height: 480.25))
    require(catalogWindowMatches(allowOffscreen: true, exactBounds: true), "fractional catalog frame rejected")

    for changed in [
        CGRect(x: -101, y: 200, width: 640, height: 480),
        CGRect(x: -99.5, y: 200, width: 640, height: 480),
        CGRect(x: -100, y: 201, width: 640, height: 480),
        CGRect(x: -100, y: 200, width: 641, height: 480),
        CGRect(x: -100, y: 200, width: 640, height: 481),
    ] {
        observe(changed)
        require(!catalogWindowMatches(allowOffscreen: true, exactBounds: true), "changed integer frame accepted")
        require(catalogWindowMatches(allowOffscreen: true), "AX bounds tolerance changed")
    }
    observe(expectedBounds)
    testRemoteCatalog(observedCatalog[0])
    print("passed")
}

func testRemoteCatalog(_ original: [String: Any]) {
    func require(_ condition: Bool, _ message: String) {
        guard condition else { fail(message) }
    }
    individualCatalog = []
    var bystander = original
    bystander[kCGWindowNumber as String] = NSNumber(value: windowID + 1)
    observedCatalog = [bystander, original]
    require(catalogWindowMatches(allowOffscreen: true, exactBounds: true), "remote window absent from individual lookup was rejected")
    require(!catalogWindowMatches(exactBounds: true), "remote offscreen guard bypassed")
    for (key, value): (String, Any) in [
        (kCGWindowNumber as String, NSNumber(value: windowID + 1)),
        (kCGWindowOwnerPID as String, NSNumber(value: pid + 1)),
        (kCGWindowName as String, "Renamed"),
        (kCGWindowBounds as String, CGRect(x: -110, y: 200, width: 640, height: 480).dictionaryRepresentation),
    ] {
        var changed = original
        changed[key] = value
        observedCatalog = [changed]
        require(!catalogWindowMatches(allowOffscreen: true, exactBounds: true), "changed remote identity accepted")
    }
    for rows in [[], [original, original]] {
        observedCatalog = rows
        require(!catalogWindowMatches(allowOffscreen: true), "missing or ambiguous remote window accepted")
    }
    observedCatalog = [original]
    catalogUnavailable = true
    require(!catalogWindowMatches(allowOffscreen: true), "unavailable remote catalog accepted")
    catalogUnavailable = false
    individualCatalog = [bystander]
    require(catalogWindowMatches(allowOffscreen: true), "unrelated individual row hid the current window")
    var conflict = original
    conflict[kCGWindowOwnerPID as String] = NSNumber(value: pid + 1)
    individualCatalog = [conflict]
    require(!catalogWindowMatches(allowOffscreen: true), "full catalog overrode a conflicting individual identity")
    individualCatalog = [original, original]
    require(!catalogWindowMatches(allowOffscreen: true), "ambiguous individual window accepted")
}
