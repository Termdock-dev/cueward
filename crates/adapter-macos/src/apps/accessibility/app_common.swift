import Darwin

alarm(30)
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    let status = AXUIElementCopyAttributeValue(element, name as CFString, &value)
    switch status {
    case .success: return value
    case .attributeUnsupported, .noValue: return nil
    default: fail("app AX read failed for \(name) (\(status.rawValue)); inspect again")
    }
}

func processIdentity(_ pid: pid_t) -> [String: Any] {
    var info = proc_bsdinfo()
    let size = Int32(MemoryLayout<proc_bsdinfo>.size)
    guard pid > 0, proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, size) == size else {
        fail("application process is unavailable; list apps again")
    }
    // PROC_PIDPATHINFO_MAXSIZE is a C expression macro unavailable to Swift.
    var buffer = [CChar](repeating: 0, count: 4 * Int(MAXPATHLEN))
    guard proc_pidpath(pid, &buffer, UInt32(buffer.count)) > 0 else { fail("application executable is unavailable") }
    return ["pid": pid, "start_seconds": info.pbi_start_tvsec,
            "start_micros": info.pbi_start_tvusec, "executable": String(cString: buffer)]
}

func elementPID(_ element: AXUIElement) -> pid_t {
    var result: pid_t = 0
    guard AXUIElementGetPid(element, &result) == .success, result > 0 else { fail("AX receiver process is unavailable") }
    return result
}

func appNode(_ element: AXUIElement) -> [String: Any] {
    var unavailable: [String] = []
    var node = describeElement(element) { element, name in
        var value: CFTypeRef?
        let status = AXUIElementCopyAttributeValue(element, name as CFString, &value)
        switch status {
        case .success: return value
        case .attributeUnsupported, .noValue: return nil
        // Preserve traversable containers with unavailable descriptive attributes.
        // Missing roles and communication failures still stop the observation.
        case .failure where name != kAXRoleAttribute: unavailable.append(name); return nil
        default: fail("app AX node read failed for \(name) (\(status.rawValue)); inspect again")
        }
    }
    guard node["role"] as? String != "unknown" else { fail("AX node has no readable role") }
    if !unavailable.isEmpty {
        node["unavailable_attributes"] = unavailable.sorted()
        node["settable_value"] = false
        node["actions"] = [String]()
    }
    let receiver = elementPID(element)
    node["receiver_pid"] = receiver
    node["receiver_instance"] = processIdentity(receiver)
    return node
}

func ensureUnlocked() {
    guard let session = CGSessionCopyCurrentDictionary() as? [String: Any] else { fail("desktop session is unavailable") }
    guard session["CGSSessionScreenIsLocked"] as? Bool != true else {
        fail("desktop is locked; app AX exploration requires an unlocked session")
    }
}

let data = FileHandle.standardInput.readDataToEndOfFile()
guard let request = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
      let pid = request["pid"] as? Int32, pid > 0 else { fail("invalid app AX request") }
guard AXIsProcessTrusted() else { fail("Accessibility permission is required", code: 3) }
ensureUnlocked()
let instance = processIdentity(pid)
let application = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(application, 2)
