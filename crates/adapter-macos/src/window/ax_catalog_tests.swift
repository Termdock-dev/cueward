import ApplicationServices
import Foundation

// Supply catalog observations without depending on a display layout or AX permission.
var observedCatalog: [[String: Any]] = []
func CGWindowListCopyWindowInfo(_ options: CGWindowListOption, _ relativeToWindow: CGWindowID) -> CFArray? {
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
    print("passed")
}
