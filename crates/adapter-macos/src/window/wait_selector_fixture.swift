import Foundation

final class AXUIElement {
    let role: String
    init(_ role: String) { self.role = role }
}
let kAXRoleAttribute = "role", kAXTitleAttribute = "title", kAXDescriptionAttribute = "description"
let kAXIdentifierAttribute = "identifier", kAXEnabledAttribute = "enabled"
let kAXSubroleAttribute = "subrole", kAXValueAttribute = "value", kAXChildrenAttribute = "children"
let scenario = CommandLine.arguments[1]
func fail(_ message: String) -> Never { fputs(message + "\n", stderr); exit(1) }
func text(_ value: Any?) -> String? { value as? String }
func elements(_ element: AXUIElement, _ name: String) -> [AXUIElement] { [] }
func checkedAttribute(_ element: AXUIElement, _ name: String) -> Any? {
    if name == kAXRoleAttribute {
        if scenario == "role-failure" { fail("required role failed") }
        return element.role
    }
    if name == kAXTitleAttribute { return scenario == "name-description-failure" ? nil : "Editor" }
    if name == kAXDescriptionAttribute { fail("description failed") }
    if name == kAXIdentifierAttribute { fail("identifier failed") }
    return nil
}
