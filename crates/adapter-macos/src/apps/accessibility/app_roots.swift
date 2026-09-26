func optionalElement(_ element: AXUIElement, _ key: String) -> AXUIElement? {
    guard let raw = attribute(element, key) else { return nil }
    guard CFGetTypeID(raw) == AXUIElementGetTypeID() else { fail("invalid AX root value") }
    return (raw as! AXUIElement)
}

struct AppRoots {
    let windows: [AXUIElement]
    let menu: AXUIElement?
    let context: String

    init() {
        var found = elements(application, kAXWindowsAttribute)
        let main = optionalElement(application, kAXMainWindowAttribute)
        let focused = optionalElement(application, kAXFocusedWindowAttribute)
        for element in [main, focused].compactMap({ $0 }) {
            if !found.contains(where: { CFEqual($0, element) }) { found.append(element) }
        }
        guard found.count <= 64 else { fail("app has too many AX roots to bind safely") }
        windows = found
        menu = optionalElement(application, kAXMenuBarAttribute)
        context = fingerprint([
            "app": instance,
            "windows": found.map { rootFingerprint($0) },
            "main": main.map { rootFingerprint($0) } as Any? ?? NSNull(),
            "focused": focused.map { rootFingerprint($0) } as Any? ?? NSNull(),
        ], parent: "")
    }

    func root(_ ref: String) -> AXUIElement {
        if ref == "menu", let menu { return menu }
        guard ref.hasPrefix("w"), let index = Int(ref.dropFirst()), windows.indices.contains(index) else {
            fail("app AX root disappeared; inspect the app again")
        }
        let element = windows[index]
        let identity = rootFingerprint(element)
        guard windows.filter({ rootFingerprint($0) == identity }).count == 1 else {
            fail("app AX root identity is ambiguous")
        }
        return element
    }
}

func rootFingerprint(_ element: AXUIElement) -> String {
    fingerprint(appNode(element), parent: "")
}

func resolve(_ reference: String, _ roots: AppRoots) -> (AXUIElement, String, String) {
    let parts = reference.split(separator: ".", omittingEmptySubsequences: false).map(String.init)
    guard let first = parts.first, parts.count <= 13 else { fail("invalid app element ref") }
    var element = roots.root(first)
    let rootID = rootFingerprint(element)
    var parent = roots.context
    for part in parts.dropFirst() {
        guard let index = Int(part), index >= 0 else { fail("invalid app element ref") }
        let children = elements(element, kAXChildrenAttribute)
        guard index < children.count else { fail("app element disappeared; inspect again") }
        parent = fingerprint(appNode(element), parent: parent)
        element = children[index]
    }
    return (element, parent, rootID)
}

func validateObservation(_ context: String) {
    ensureUnlocked()
    guard fingerprint(processIdentity(pid), parent: "") == fingerprint(instance, parent: ""),
          AppRoots().context == context else { fail("app context changed; inspect again") }
}
