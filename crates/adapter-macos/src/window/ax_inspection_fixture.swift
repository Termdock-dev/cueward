// A deterministic tree runs the production traversal without desktop permissions.
import Foundation

final class AXUIElement {
    let name: String
    let children: [AXUIElement]
    init(_ name: String, _ children: [AXUIElement] = []) {
        self.name = name
        self.children = children
    }
}
let fixtureRoot = AXUIElement("window", [
    AXUIElement("editor", [AXUIElement("input"), AXUIElement("button")]),
    AXUIElement("other"),
])
let kAXChildrenAttribute = "children"
let windowID = 42
let pid = 123
func bindWindow() -> AXUIElement { fixtureRoot }
func validateCatalogWindow() {}
func elements(_ element: AXUIElement, _ attribute: String) -> [AXUIElement] { element.children }
func describeElement(_ element: AXUIElement) -> [String: Any] { ["name": element.name] }
func fingerprint(_ node: [String: Any], parent: String) -> String {
    parent + "/" + (node["name"] as! String)
}
func fail(_ message: String) -> Never {
    fputs(message, stderr)
    exit(1)
}
func emit(_ value: [String: Any]) {
    let data = try! JSONSerialization.data(withJSONObject: value)
    print(String(data: data, encoding: .utf8)!)
}
