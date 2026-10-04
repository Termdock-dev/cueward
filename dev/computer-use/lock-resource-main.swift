import AppKit
import Darwin

@MainActor final class LockProbe {
    let pid: pid_t
    let windowID: UInt32
    let ax: RetainedLockAX
    var capture: RetainedLockCapture?
    var captureError: LockCaptureFailure?
    var sawLock = false
    var testedActions = Set<String>()
    let allowInput: Bool
    let parent = getppid()

    init(_ config: [String: Any]) throws {
        guard let rawPID = config["pid"] as? Int32, rawPID > 0,
              let nativeID = config["window_id"] as? UInt32, nativeID > 0,
              let enabled = config["diagnostic_input"] as? Bool,
              let lifetime = config["lifetime"] as? Double, (1...600).contains(lifetime) else {
            throw LockResourceError("invalid owned resource configuration")
        }
        pid = rawPID; windowID = nativeID; allowInput = enabled
        ax = try RetainedLockAX(pid: pid, windowID: windowID)
        DispatchQueue.global().asyncAfter(deadline: .now() + lifetime) { exit(4) }
    }

    func prepare() async {
        do { capture = try await RetainedLockCapture.start(pid: pid, windowID: windowID) }
        catch { captureError = error as? LockCaptureFailure ?? LockCaptureFailure(description: "capture setup failed") }
        emit(["kind": "ready", "schema": 1, "pid": pid, "window_id": windowID,
              "retained_ax": true, "retained_stream": capture != nil,
              "capture_error": captureError?.description ?? "",
              "capture_status": captureError?.sample().status ?? "ready", "diagnostic_input": allowInput,
              "uptime": ProcessInfo.processInfo.systemUptime])
    }

    func phase(_ locked: Bool?) -> String {
        guard let locked else { return "unknown" }
        if locked { sawLock = true; return "locked" }
        return sawLock ? "after_unlock" : "before_lock"
    }

    func unavailableCapture() -> [String: Any] {
        (captureError ?? LockCaptureFailure(description: "capture setup unavailable")).sample().record
    }

    func sample() async {
        let begin = ProcessInfo.processInfo.systemUptime
        let locked = resourceLocked()
        let retained = ax.read(retained: true)
        let fresh = ax.read(retained: false)
        let stream = capture?.snapshot().record ?? unavailableCapture()
        let screenshot: [String: Any]
        if let capture { screenshot = await capture.screenshot().record }
        else { screenshot = unavailableCapture() }
        emit(["kind": "sample", "phase": phase(locked), "locked": locked.map { $0 as Any } ?? NSNull(),
              "locked_end": resourceLocked().map { $0 as Any } ?? NSNull(),
              "begin_uptime": begin, "end_uptime": ProcessInfo.processInfo.systemUptime,
              "observations": ["retained_ax": retained, "new_ax": fresh,
                               "retained_stream": stream, "new_screenshot": screenshot]])
    }

    func action(_ command: [String: Any]) throws {
        let locked = resourceLocked()
        let currentPhase = phase(locked)
        guard allowInput, let retained = command["retained"] as? Bool,
              let expected = command["phase"] as? String, expected == currentPhase, locked != nil else {
            throw LockResourceError("diagnostic input not enabled or phase changed")
        }
        let key = currentPhase + (retained ? ":retained_ax" : ":new_ax")
        guard testedActions.insert(key).inserted else {
            throw LockResourceError("diagnostic action already attempted; no replay")
        }
        let begin = ProcessInfo.processInfo.systemUptime
        let observation = ax.press(retained: retained)
        emit(["kind": "action", "phase": currentPhase, "route": retained ? "retained_ax" : "new_ax",
              "begin_uptime": begin, "end_uptime": ProcessInfo.processInfo.systemUptime,
              "locked": locked.map { $0 as Any } ?? NSNull(),
              "locked_end": resourceLocked().map { $0 as Any } ?? NSNull(), "observation": observation])
    }

    func run() async throws {
        await prepare()
        while let line = readLine() {
            guard line.utf8.count <= 4096, kill(pid, 0) == 0, getppid() == parent,
                  let data = line.data(using: .utf8),
                  let command = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                throw LockResourceError("invalid control or owned process unavailable")
            }
            switch command["command"] as? String {
            case "sample": await sample()
            case "action": try action(command)
            case "stop": await finish(reason: "stop"); return
            default: throw LockResourceError("unknown diagnostic command")
            }
        }
        await finish(reason: "control_closed")
    }

    func finish(reason: String) async {
        await capture?.stop()
        emit(["kind": "finished", "reason": reason,
              "capture_stop": capture?.snapshot().record ?? ["status": "not_started"]])
    }
}

func emit(_ value: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) else { exit(3) }
    FileHandle.standardOutput.write(data); FileHandle.standardOutput.write(Data([10]))
}

@main struct LockResourceMain {
    @MainActor static func main() async {
        // Initialize AppKit's WindowServer connection without becoming a foreground app.
        NSApplication.shared.setActivationPolicy(.prohibited)
        do {
            guard CommandLine.arguments.count == 2,
                  let data = CommandLine.arguments[1].data(using: .utf8),
                  let config = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                throw LockResourceError("expected one JSON configuration")
            }
            try await LockProbe(config).run()
        } catch {
            emit(["kind": "error", "error": String(describing: error)])
            exit(1)
        }
    }
}
