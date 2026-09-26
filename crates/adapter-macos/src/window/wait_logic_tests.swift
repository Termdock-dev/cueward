import Foundation
func require(_ success: Bool, _ message: String) {
    if !success { fputs(message + "\n", stderr); exit(1) }
}
let selector: [String: Any] = ["role": "AXTextField", "name": "Status", "identifier": "status"]
let matching: [String: Any] = ["role": "AXTextField", "name": "Status", "identifier": "status", "value": "ready", "enabled": true]
require(selectorMatches(matching, selector), "exact selector rejected")
for key in ["role", "name", "identifier"] {
    var changed = matching; changed[key] = "different"
    require(!selectorMatches(changed, selector), "selector mismatch accepted")
}
require(evaluateWait("element-exists", [matching], complete: false, value: nil) == "matched", "observed presence lost")
require(evaluateWait("element-absent", [], complete: false, value: nil) == "pending", "truncation treated as absence")
require(evaluateWait("element-absent", [], complete: true, value: nil) == "matched", "complete absence lost")
require(evaluateWait("element-absent", [matching], complete: true, value: nil) == "pending", "present element treated as absent")
for condition in ["value-equals", "enabled"] {
    require(evaluateWait(condition, [matching, matching], complete: true, value: "ready") == "ambiguous", "duplicates accepted")
    require(evaluateWait(condition, [matching], complete: false, value: "ready") == "pending", "partial tree treated as unique")
    require(evaluateWait(condition, [], complete: true, value: "ready") == "pending", "missing node accepted")
    require(evaluateWait(condition, [matching], complete: true, value: "ready") == "matched", "unique condition rejected")
}
require(evaluateWait("value-equals", [matching], complete: true, value: "rea") == "pending", "prefix accepted")
require(evaluateWait("value-equals", [["role": "AXSecureTextField"]], complete: true, value: "") == "pending", "missing secret value accepted")
require(evaluateWait("enabled", [["enabled": false]], complete: true, value: nil) == "pending", "disabled accepted")
print("passed")
