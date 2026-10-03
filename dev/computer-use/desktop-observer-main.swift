import AppKit
import Darwin

/// All mutable state and desktop reads are confined to the main actor/run loop.
@MainActor final class ObserverLoop {
    let configuration: ObserverConfiguration
    let observer: DesktopObservation
    let initial = ProcessInfo.processInfo.systemUptime
    let parent = getppid()
    var running = true
    var reason = "stopped"
    var marks = Set<String>()
    var pending = Data()

    init(_ configuration: ObserverConfiguration) throws {
        self.configuration = configuration
        observer = try DesktopObservation(pids: configuration.pids)
    }

    func now() -> Double { (ProcessInfo.processInfo.systemUptime - initial) * 1000 }

    func emit(_ value: [String: Any]) {
        do {
            let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
            FileHandle.standardOutput.write(data)
            FileHandle.standardOutput.write(Data([10]))
        } catch { running = false; reason = "serialization_error" }
    }

    func sample() {
        do {
            var value = try observer.sample(ms: now())
            value["sample_end_ms"] = now()
            emit(value)
        }
        catch { emit(["type": "observation_error", "ms": now(), "error": String(describing: error)]) }
    }

    func tick() {
        guard running else { return }
        sample()
        if now() >= configuration.durationMS { running = false; reason = "deadline" }
        if getppid() != parent { running = false; reason = "caller_exited" }
    }

    func readControls() {
        guard running else { return }
        // Read only available bytes; a partial stdin line cannot block sampling.
        var bytes = [UInt8](repeating: 0, count: 4096)
        let count = read(STDIN_FILENO, &bytes, bytes.count)
        if count <= 0 { running = false; reason = "control_closed"; return }
        pending.append(contentsOf: bytes.prefix(count))
        do {
            while let newline = pending.firstIndex(of: 10) {
                let line = Data(pending.prefix(upTo: newline))
                pending.removeSubrange(...newline)
                let (command, id) = try observerCommand(line)
                if command == "stop" { running = false; return }
                guard let id, marks.insert(id).inserted else {
                    throw ObservationError.invalid("duplicate marker ID")
                }
                sample()
                emit(["type": "mark", "id": id, "ms": now()])
                sample()
            }
            if pending.count > 4096 { throw ObservationError.invalid("control line exceeds 4096 bytes") }
        } catch {
            emit(["type": "control_error", "ms": now(), "error": String(describing: error)])
            running = false; reason = "control_error"
        }
    }

    func run() {
        emit(["type": "ready", "schema": 1, "clock": "observer_system_uptime_ms",
              "receiver_pids": configuration.pids, "interval_ms": configuration.intervalMS,
              "duration_ms": configuration.durationMS,
              "macos": ProcessInfo.processInfo.operatingSystemVersionString])
        sample()
        let activation = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main
        ) { [self] notification in
            let pid = (notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication)?.processIdentifier
            MainActor.assumeIsolated {
                if let pid { emit(["type": "activation", "ms": now(), "pid": pid]) }
            }
        }
        let timer = Timer(timeInterval: configuration.intervalMS / 1000, repeats: true) { [self] _ in
            MainActor.assumeIsolated { tick() }
        }
        RunLoop.main.add(timer, forMode: .default)
        let controls = DispatchSource.makeReadSource(fileDescriptor: STDIN_FILENO, queue: .main)
        controls.setEventHandler { [self] in MainActor.assumeIsolated { readControls() } }
        controls.resume()
        while running { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
        timer.invalidate()
        controls.cancel()
        NSWorkspace.shared.notificationCenter.removeObserver(activation)
        sample()
        emit(["type": "finished", "ms": now(), "reason": reason])
    }
}

@main struct ObserverMain {
    static func main() {
        do {
            guard CommandLine.arguments.count == 2,
                  let data = CommandLine.arguments[1].data(using: .utf8),
                  let value = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                throw ObservationError.invalid("expected one JSON configuration argument")
            }
            try ObserverLoop(ObserverConfiguration(value)).run()
        } catch {
            fputs("desktop observer: \(error)\n", stderr)
            exit(1)
        }
    }
}
