import ApplicationServices
import Darwin
import Foundation

struct DialogAXError: Error, CustomStringConvertible {
    let description: String
    init(_ value: String) { description = value }
}

func dialogAttribute(_ element: AXUIElement, _ key: String, optional: Bool = false) throws -> CFTypeRef? {
    var value: CFTypeRef?
    let status = AXUIElementCopyAttributeValue(element, key as CFString, &value)
    if optional && (status == .attributeUnsupported || status == .noValue) { return nil }
    guard status == .success else { throw DialogAXError("AX observation failed: \(status.rawValue)") }
    return value
}

func dialogElements(_ element: AXUIElement, _ key: String, optional: Bool = false) throws -> [AXUIElement] {
    guard let value = try dialogAttribute(element, key, optional: optional) else { return [] }
    guard let values = value as? [AXUIElement] else { throw DialogAXError("invalid AX element array") }
    return values
}

func dialogRoots(_ application: AXUIElement) throws -> [AXUIElement] {
    var roots = try dialogElements(application, kAXWindowsAttribute)
    for key in [kAXMainWindowAttribute, kAXFocusedWindowAttribute] {
        if let value = try dialogAttribute(application, key, optional: true) {
            guard CFGetTypeID(value) == AXUIElementGetTypeID() else { throw DialogAXError("invalid optional AX root") }
            let element = value as! AXUIElement
            if !roots.contains(where: { CFEqual($0, element) }) { roots.append(element) }
        }
    }
    guard roots.count <= 64 else { throw DialogAXError("too many AX roots") }
    return roots
}

struct DialogAXReader {
    typealias Lookup = @convention(c) (AXUIElement, UnsafeMutablePointer<CGWindowID>) -> AXError
    let lookup: Lookup
    let pid: pid_t
    let application: AXUIElement

    init(pid: pid_t) throws {
        guard pid > 0, AXIsProcessTrusted(), kill(pid, 0) == 0,
              let session = CGSessionCopyCurrentDictionary() as? [String: Any],
              session["CGSSessionScreenIsLocked"] as? Bool != true else {
            throw DialogAXError("owned process, AX permission and unlocked session required")
        }
        guard let symbol = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "_AXUIElementGetWindow") else {
            throw DialogAXError("direct native AX window lookup unavailable")
        }
        lookup = unsafeBitCast(symbol, to: Lookup.self)
        self.pid = pid
        application = AXUIElementCreateApplication(pid)
        guard AXUIElementSetMessagingTimeout(application, 0.3) == .success else {
            throw DialogAXError("cannot set bounded AX timeout")
        }
    }

    func identity(_ element: AXUIElement, ref: String, requireFixture: Bool = true) throws -> [String: Any] {
        var receiver: pid_t = 0
        guard AXUIElementGetPid(element, &receiver) == .success, receiver > 0,
              (!requireFixture || receiver == pid), kill(receiver, 0) == 0,
              let role = try dialogAttribute(element, kAXRoleAttribute) as? String else {
            throw DialogAXError("AX receiver or role differs from owned fixture")
        }
        return ["receiver_pid": receiver, "ref": ref, "role": role]
    }

    func nativeID(_ element: AXUIElement) -> CGWindowID? {
        var value: CGWindowID = 0
        guard lookup(element, &value) == .success, value > 0 else { return nil }
        return value
    }

    func matches(_ element: AXUIElement, id: CGWindowID, identifier: String) throws -> Bool {
        guard nativeID(element) == id else { return false }
        return (try dialogAttribute(element, kAXIdentifierAttribute, optional: true) as? String) == identifier
    }

    func sheet(_ owner: AXUIElement, parent: String, id: CGWindowID, identifier: String) throws -> ([String: Any], [pid_t]) {
        var queue: [(AXUIElement, String, Int)] = [(owner, parent, 0)]
        var found: [[String: Any]] = []
        var candidates: [[String: Any]] = []
        var receivers: [DialogReceiverObservation] = []
        var index = 0
        while index < queue.count && index < 500 {
            let (element, ref, depth) = queue[index]
            index += 1
            var observedReceiver: pid_t = 0
            guard AXUIElementGetPid(element, &observedReceiver) == .success else {
                throw DialogAXError("descendant receiver identity unavailable")
            }
            receivers.append(DialogReceiverObservation(ref: ref, pid: observedReceiver,
                                                       nativeWindowID: nativeID(element)))
            if (try dialogAttribute(element, kAXRoleAttribute) as? String) == "AXSheet" {
                var receiver: pid_t = 0
                _ = AXUIElementGetPid(element, &receiver)
                candidates.append(["ref": ref, "receiver_pid": receiver,
                    "window_id": nativeID(element) as Any? ?? NSNull(),
                    "identifier": (try dialogAttribute(element, kAXIdentifierAttribute, optional: true) as? String) as Any? ?? NSNull()])
            }
            // Remote NSSavePanel uses a service-owned AXSheet and exposes "save-panel",
            // not its host NSWindow's configured UUID. Direct native ID + AXWindow
            // CFEqual(owner) is the association; never fall back to label or position.
            if nativeID(element) == id {
                let root = try identity(element, ref: ref, requireFixture: false)
                if root["role"] as? String == "AXSheet" {
                    guard let attached = try dialogAttribute(element, kAXWindowAttribute),
                          CFGetTypeID(attached) == AXUIElementGetTypeID(), CFEqual(attached, owner) else {
                        throw DialogAXError("save sheet is not independently attached to owner")
                    }
                    found.append(root)
                }
            }
            if depth < 12 {
                for (childIndex, child) in try dialogElements(element, kAXChildrenAttribute, optional: true).enumerated() {
                    queue.append((child, "\(ref).\(childIndex)", depth + 1))
                }
            }
        }
        guard index == queue.count, found.count == 1 else {
            let details: [String: Any] = ["kind": "sheet_binding_unavailable", "expected_window_id": id,
                "expected_identifier": identifier, "visited": index, "queued": queue.count, "candidates": candidates]
            if let data = try? JSONSerialization.data(withJSONObject: details, options: [.sortedKeys]) {
                FileHandle.standardError.write(data); FileHandle.standardError.write(Data("\n".utf8))
            }
            throw DialogAXError("save sheet native identity/descendant binding unavailable or ambiguous")
        }
        let root = found[0]
        let reference = root["ref"] as! String
        let pids = try boundDialogSheetReceivers(receivers, hostPID: pid, root: reference,
                                                panelID: id, isAlive: { kill($0, 0) == 0 })
        guard pids.contains(root["receiver_pid"] as! pid_t) else {
            throw DialogAXError("sheet receiver has no direct native window association")
        }
        return (root, pids.sorted())
    }

    func observe(_ request: [String: Any]) throws -> [String: Any] {
        guard let inventory = request["windows"] as? [[String: Any]], inventory.count == 2 else {
            throw DialogAXError("two owned native windows required")
        }
        let roots = try dialogRoots(application)
        var windows: [[String: Any]] = []
        var targetElement: AXUIElement?
        var targetRoot: [String: Any]?
        for window in inventory {
            guard let id = window["window_id"] as? UInt32, id > 0,
                  window["pid"] as? Int32 == pid,
                  let identifier = window["ax_identifier"] as? String, !identifier.isEmpty,
                  let file = window["file"] as? String else { throw DialogAXError("invalid owned window inventory") }
            var found: [(Int, AXUIElement)] = []
            for (index, element) in roots.enumerated() {
                if try matches(element, id: id, identifier: identifier) { found.append((index, element)) }
            }
            guard found.count == 1 else { throw DialogAXError("owned document native identity is not unique") }
            let (index, element) = found[0]
            let root = try identity(element, ref: "w\(index)")
            guard root["role"] as? String == "AXWindow" else { throw DialogAXError("document root is not AXWindow") }
            windows.append(window.merging(["root": root], uniquingKeysWith: { _, new in new }))
            if file == request["target_file"] as? String { targetElement = element; targetRoot = root }
        }
        guard let targetElement, let targetRoot else { throw DialogAXError("owned target document missing") }
        var dialogValue: Any = NSNull()
        if let dialog = request["dialog"] as? [String: Any] {
            guard dialog["kind"] as? String == "save", let id = dialog["window_id"] as? UInt32, id > 0,
                  let identifier = dialog["ax_identifier"] as? String, !identifier.isEmpty,
                  let owner = dialog["owner_window"] as? [String: Any], owner["pid"] as? Int32 == pid,
                  let targetID = windows.first(where: { $0["file"] as? String == request["target_file"] as? String })?["window_id"] as? UInt32,
                  owner["window_id"] as? UInt32 == targetID else { throw DialogAXError("save purpose/owner not bound to target") }
            var observed = dialog
            let canonicalOwner: [String: Any] = ["pid": pid, "window_id": targetID]
            observed["owner_window"] = canonicalOwner
            if dialog["mode"] as? String == "sheet" {
                let (root, receivers) = try sheet(targetElement, parent: targetRoot["ref"] as! String, id: id, identifier: identifier)
                observed["root"] = root
                observed["parent_root"] = targetRoot
                observed["native_binding"] = ["source": "native_ax_window_binding", "window_id": id,
                    "owner_window": canonicalOwner, "owner_window_cf_equal": true, "receiver_pids": receivers]
            } else if dialog["mode"] as? String == "window" {
                var found: [[String: Any]] = []
                for (index, element) in roots.enumerated() {
                    if try matches(element, id: id, identifier: identifier) { found.append(try identity(element, ref: "w\(index)")) }
                }
                guard found.count == 1, ["AXWindow", "AXDialog"].contains(found[0]["role"] as? String ?? "") else {
                    throw DialogAXError("standalone save panel native root unavailable or ambiguous")
                }
                observed["root"] = found[0]
            } else { throw DialogAXError("unknown native save dialog mode") }
            dialogValue = observed
        }
        let finalRoots = try dialogRoots(application)
        guard roots.count == finalRoots.count, zip(roots, finalRoots).allSatisfy({ CFEqual($0.0, $0.1) }) else {
            throw DialogAXError("AX roots changed during native binding observation")
        }
        var auxiliary: [[String: Any]] = []
        if let dialog = dialogValue as? [String: Any], let binding = dialog["native_binding"] as? [String: Any],
           let receivers = binding["receiver_pids"] as? [Int32] {
            for receiver in receivers where receiver != pid {
                auxiliary.append(["pid": receiver, "window_id": dialog["window_id"]!,
                    "owner_window": dialog["owner_window"]!, "source": "native_ax_window_binding"])
            }
        }
        return ["pid": pid, "windows": windows, "dialog": dialogValue, "auxiliary_receivers": auxiliary,
                "lookup_api": "_AXUIElementGetWindow", "uptime": ProcessInfo.processInfo.systemUptime]
    }
}

@main struct WindowDialogAXMain {
    static func main() {
        do {
            let input = FileHandle.standardInput.readDataToEndOfFile()
            guard input.count <= 1_048_576,
                  let request = try JSONSerialization.jsonObject(with: input) as? [String: Any],
                  let pid = request["pid"] as? Int32 else { throw DialogAXError("invalid bounded observation request") }
            let value = try DialogAXReader(pid: pid).observe(request)
            let output = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
            FileHandle.standardOutput.write(output)
        } catch {
            FileHandle.standardError.write(Data((String(describing: error) + "\n").utf8))
            exit(1)
        }
    }
}
