import AppKit
import ApplicationServices
import Darwin

struct LockResourceError: Error, CustomStringConvertible {
    let description: String
    init(_ message: String) { description = message }
}

func resourceLocked() -> Bool? {
    guard let session = CGSessionCopyCurrentDictionary() as? [String: Any] else { return nil }
    return session["CGSSessionScreenIsLocked"] as? Bool ?? false
}

/// Handles are retained, not a claim about private server-side transport identity.
@MainActor final class RetainedLockAX {
    let pid: pid_t
    let windowID: CGWindowID
    let application: AXUIElement
    let label: AXUIElement
    let button: AXUIElement

    init(pid: pid_t, windowID: CGWindowID) throws {
        self.pid = pid; self.windowID = windowID
        guard AXIsProcessTrusted(), resourceLocked() == false else {
            throw LockResourceError("AX permission and an unlocked setup are required")
        }
        let timeout = AXUIElementSetMessagingTimeout(AXUIElementCreateSystemWide(), 1)
        guard timeout == .success else { throw LockResourceError("cannot set bounded AX timeout: \(timeout.rawValue)") }
        application = AXUIElementCreateApplication(pid)
        let window = try Self.window(application, pid: pid, windowID: windowID)
        label = try Self.find(window, identifier: "lock-sequence")
        button = try Self.find(window, identifier: "lock-action")
    }

    static func attribute(_ element: AXUIElement, _ name: String) throws -> CFTypeRef {
        var value: CFTypeRef?
        let status = AXUIElementCopyAttributeValue(element, name as CFString, &value)
        guard status == .success, let value else { throw AXReadError(status.rawValue) }
        return value
    }

    static func window(_ application: AXUIElement, pid: pid_t, windowID: CGWindowID) throws -> AXUIElement {
        let windows = try attribute(application, kAXWindowsAttribute) as? [AXUIElement] ?? []
        typealias Lookup = @convention(c) (AXUIElement, UnsafeMutablePointer<CGWindowID>) -> AXError
        guard let symbol = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "_AXUIElementGetWindow") else {
            throw LockResourceError("native AX window identity lookup unavailable")
        }
        let lookup = unsafeBitCast(symbol, to: Lookup.self)
        let matching = windows.filter { element in
            var owner: pid_t = 0; var observed: CGWindowID = 0
            return AXUIElementGetPid(element, &owner) == .success && owner == pid
                && lookup(element, &observed) == .success && observed == windowID
        }
        guard matching.count == 1 else { throw LockResourceError("owned native AX window not uniquely exposed") }
        return matching[0]
    }

    static func find(_ window: AXUIElement, identifier: String) throws -> AXUIElement {
        var queue = [window]; var index = 0
        while index < queue.count && index < 64 {
            let element = queue[index]; index += 1
            if (try? attribute(element, kAXIdentifierAttribute) as? String) == identifier { return element }
            if let children = try? attribute(element, kAXChildrenAttribute) as? [AXUIElement] { queue += children }
        }
        throw LockResourceError("owned diagnostic element unavailable")
    }

    func newElement(identifier: String) throws -> AXUIElement {
        try Self.find(Self.window(AXUIElementCreateApplication(pid), pid: pid, windowID: windowID), identifier: identifier)
    }

    func read(retained: Bool) -> [String: Any] {
        do {
            let element = retained ? label : try newElement(identifier: "lock-sequence")
            let raw = try Self.attribute(element, kAXValueAttribute) as? String
            guard let raw, raw.hasPrefix("SEQ:"), let sequence = Int(raw.dropFirst(4)), sequence > 0 else {
                throw LockResourceError("invalid synthetic AX sequence")
            }
            return ["status": "decoded", "sequence": sequence, "api_code": 0,
                    "uptime": ProcessInfo.processInfo.systemUptime]
        } catch { return failure(error) }
    }

    func press(retained: Bool) -> [String: Any] {
        var attempted = false
        do {
            guard kill(pid, 0) == 0, NSWorkspace.shared.frontmostApplication?.processIdentifier != pid else {
                throw LockResourceError("owned receiver exited or became foreground")
            }
            // Revalidate the same native window, even for the retained target trial.
            _ = try Self.window(AXUIElementCreateApplication(pid), pid: pid, windowID: windowID)
            let element = retained ? button : try newElement(identifier: "lock-action")
            var owner: pid_t = 0
            guard AXUIElementGetPid(element, &owner) == .success, owner == pid else {
                throw LockResourceError("diagnostic action target ownership changed")
            }
            guard (try Self.attribute(element, kAXEnabledAttribute) as? Bool) == true,
                  (try Self.attribute(element, kAXRoleAttribute) as? String) == kAXButtonRole else {
                throw LockResourceError("diagnostic button unavailable or disabled")
            }
            attempted = true
            let result = AXUIElementPerformAction(element, kAXPressAction as CFString)
            return ["status": result == .success ? "sent_unverified" : "error", "api_code": result.rawValue,
                    "attempted": true, "stage": "dispatch", "uptime": ProcessInfo.processInfo.systemUptime]
        } catch {
            var value = failure(error)
            value["attempted"] = attempted; value["stage"] = "prevalidation_failed"
            return value
        }
    }

    private func failure(_ error: Error) -> [String: Any] {
        ["status": "error", "api_code": (error as? AXReadError)?.code ?? -1,
         "error": String(describing: error), "uptime": ProcessInfo.processInfo.systemUptime]
    }
}

struct AXReadError: Error, CustomStringConvertible {
    let code: Int32
    init(_ code: Int32) { self.code = code }
    var description: String { "AX error \(code)" }
}
