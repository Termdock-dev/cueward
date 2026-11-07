import AppKit
import CoreGraphics
import Darwin

/// Counts input by source from a listen-only tap. Key codes and characters are never read.
/// A source PID of zero is hardware; idle-time APIs cannot tell, because posted events reset them too.
final class InputAttribution: @unchecked Sendable {
    private(set) var physicalPointer = 0
    private(set) var automatedPointer = 0
    private(set) var physicalOther = 0
    private var tap: CFMachPort?

    func start() -> Bool {
        let pointer: [CGEventType] = [.mouseMoved, .leftMouseDragged, .rightMouseDragged, .otherMouseDragged]
        let other: [CGEventType] = [.leftMouseDown, .rightMouseDown, .otherMouseDown, .scrollWheel, .keyDown]
        let mask = (pointer + other).reduce(CGEventMask(0)) { $0 | (1 << CGEventMask($1.rawValue)) }
        let callback: CGEventTapCallBack = { _, type, event, context in
            if let context { Unmanaged<InputAttribution>.fromOpaque(context).takeUnretainedValue().record(type, event) }
            return Unmanaged.passUnretained(event)
        }
        guard let created = CGEvent.tapCreate(tap: .cgSessionEventTap, place: .tailAppendEventTap, options: .listenOnly,
                                              eventsOfInterest: mask, callback: callback,
                                              userInfo: Unmanaged.passUnretained(self).toOpaque()) else { return false }
        tap = created
        CFRunLoopAddSource(CFRunLoopGetMain(), CFMachPortCreateRunLoopSource(nil, created, 0), .commonModes)
        CGEvent.tapEnable(tap: created, enable: true)
        return true
    }

    private func record(_ type: CGEventType, _ event: CGEvent) {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            if let tap { CGEvent.tapEnable(tap: tap, enable: true) }
            return
        }
        let physical = event.getIntegerValueField(.eventSourceUnixProcessID) == 0
        switch type {
        case .mouseMoved, .leftMouseDragged, .rightMouseDragged, .otherMouseDragged:
            if physical { physicalPointer += 1 } else { automatedPointer += 1 }
        case .keyDown:
            if physical { physicalOther += 1 }
        default:
            if physical { physicalOther += 1 } else { automatedPointer += 1 }
        }
    }
}

/// Read-only desktop observation. No activation, input, capture or window enumeration.
@MainActor final class DesktopObservation {
    typealias Connection = @convention(c) () -> Int32
    typealias DisplaySpaces = @convention(c) (Int32) -> Unmanaged<CFArray>?
    private let connection: Int32
    private let displaySpaces: DisplaySpaces
    private let receivers: [NSRunningApplication]
    private let library: UnsafeMutableRawPointer
    private let input = InputAttribution()
    private let inputAvailable: Bool
    private let panelServices: Bool
    private let uptimeOrigin: Double
    private let timebase: mach_timebase_info_data_t
    private var listing = KernelProcessListing()
    private var tracker = PanelServiceTracker()
    private var serviceApps: [Int32: NSRunningApplication] = [:]

    init(pids: [Int32], panelServices: Bool, uptimeOrigin: Double) throws {
        self.panelServices = panelServices
        self.uptimeOrigin = uptimeOrigin
        var timebase = mach_timebase_info_data_t()
        guard mach_timebase_info(&timebase) == KERN_SUCCESS, timebase.denom > 0 else {
            throw ObservationError.invalid("kernel timebase is unavailable")
        }
        self.timebase = timebase
        guard let library = dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_LAZY),
              let connectionSymbol = dlsym(library, "SLSMainConnectionID"),
              let spacesSymbol = dlsym(library, "SLSCopyManagedDisplaySpaces") else {
            throw ObservationError.invalid("read-only Space observation is unavailable")
        }
        self.library = library
        connection = unsafeBitCast(connectionSymbol, to: Connection.self)()
        displaySpaces = unsafeBitCast(spacesSymbol, to: DisplaySpaces.self)
        receivers = try pids.map { pid in
            guard let app = NSRunningApplication(processIdentifier: pid), !app.isTerminated else {
                throw ObservationError.invalid("owned receiver \(pid) is unavailable")
            }
            return app
        }
        inputAvailable = input.start()
    }

    private func displayUUID(_ id: CGDirectDisplayID) throws -> String {
        guard let uuid = CGDisplayCreateUUIDFromDisplayID(id)?.takeRetainedValue() else {
            throw ObservationError.invalid("display UUID is unavailable")
        }
        return CFUUIDCreateString(nil, uuid) as String
    }

    private func visibleSpaces() throws -> [String: String] {
        var count: UInt32 = 0
        guard CGGetOnlineDisplayList(0, nil, &count) == .success, count > 0, count <= 32 else {
            throw ObservationError.invalid("online display enumeration failed")
        }
        var displays = [CGDirectDisplayID](repeating: 0, count: Int(count))
        var actual: UInt32 = 0
        guard CGGetOnlineDisplayList(count, &displays, &actual) == .success, actual == count else {
            throw ObservationError.invalid("online displays changed during enumeration")
        }
        var online: [String: String] = [:]
        for display in displays {
            // Shared/mirrored Space inference is intentionally not substituted for observation.
            online[String(display)] = try displayUUID(display)
        }
        guard let rows = displaySpaces(connection)?.takeRetainedValue() as? [[String: Any]] else {
            throw ObservationError.invalid("managed display Space catalog is unavailable")
        }
        return try correlateDisplaySpaces(rows, online: online, mainUUID: displayUUID(CGMainDisplayID()))
    }

    /// Before LaunchServices registers a service it has no activation state, so it cannot be active.
    private func serviceActive(_ pid: Int32) -> Bool {
        if serviceApps[pid] == nil { serviceApps[pid] = NSRunningApplication(processIdentifier: pid) }
        return serviceApps[pid]?.isActive ?? false
    }

    private func serviceRecord(_ process: ListedProcess, firstSeen ms: Double) -> [String: Any] {
        ["pid": process.pid, "executable": panelServiceExecutable, "first_seen_ms": ms,
         "start_ms": kernelStartMS(ticks: process.startTicks, numer: timebase.numer, denom: timebase.denom,
                                   origin: uptimeOrigin)]
    }

    func sample(now: @MainActor () -> Double) throws -> [String: Any] {
        // Listing before the timestamp keeps a kernel start at or before its first-seen sample.
        let services = panelServices ? try listing.processes(named: panelServiceExecutable) : []
        let ms = now()
        guard let session = CGSessionCopyCurrentDictionary() as? [String: Any],
              let event = CGEvent(source: nil),
              let foreground = NSWorkspace.shared.frontmostApplication else {
            throw ObservationError.invalid("desktop session, pointer or foreground is unavailable")
        }
        var active: [String: Bool] = [:]
        for app in receivers {
            guard !app.isTerminated else { throw ObservationError.invalid("owned receiver exited during observation") }
            active[String(app.processIdentifier)] = app.isActive
        }
        let pointer = event.location
        let spaces = try visibleSpaces()
        // A failed sample stops above, so a service is first seen only in a sample that exists.
        let found = tracker.update(services)
        for process in found { serviceApps[process.pid] = nil }
        serviceApps = serviceApps.filter { tracker.live[$0.key] != nil }
        for process in services { active[String(process.pid)] = serviceActive(process.pid) }
        var value: [String: Any] = ["type": "sample", "ms": ms, "frontmost_pid": foreground.processIdentifier,
                                    "pointer": [pointer.x, pointer.y], "visible_spaces": spaces,
                                    "target_active": active,
                                    "session_locked": session["CGSSessionScreenIsLocked"] as? Bool == true]
        if !found.isEmpty { value["panel_services"] = found.map { serviceRecord($0, firstSeen: ms) } }
        if inputAvailable {
            value["input"] = ["physical_pointer": input.physicalPointer, "automated_pointer": input.automatedPointer,
                              "physical_other": input.physicalOther]
        }
        return value
    }
}
