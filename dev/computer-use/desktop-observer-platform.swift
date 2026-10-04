import AppKit
import CoreGraphics
import Darwin

/// Read-only desktop observation. No activation, input, capture or window enumeration.
@MainActor final class DesktopObservation {
    typealias Connection = @convention(c) () -> Int32
    typealias DisplaySpaces = @convention(c) (Int32) -> Unmanaged<CFArray>?
    private let connection: Int32
    private let displaySpaces: DisplaySpaces
    private let receivers: [NSRunningApplication]
    private let library: UnsafeMutableRawPointer

    init(pids: [Int32]) throws {
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

    func sample(ms: Double) throws -> [String: Any] {
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
        return ["type": "sample", "ms": ms, "frontmost_pid": foreground.processIdentifier,
                "pointer": [pointer.x, pointer.y], "visible_spaces": try visibleSpaces(),
                "target_active": active,
                "session_locked": session["CGSSessionScreenIsLocked"] as? Bool == true]
    }
}
