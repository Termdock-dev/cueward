import Foundation

enum LaunchState<Value> {
    case pending
    case completed(Value)
    case timedOut
}

// All state transitions are protected by the lock, including timeout versus completion.
final class LaunchCompletion<Value>: @unchecked Sendable {
    private let lock = NSLock()
    private let deadline: TimeInterval
    private var state: LaunchState<Value> = .pending

    init(deadline: TimeInterval) { self.deadline = deadline }

    @discardableResult
    func complete(_ value: Value, now: TimeInterval = ProcessInfo.processInfo.systemUptime) -> Bool {
        lock.lock(); defer { lock.unlock() }
        guard case .pending = state else { return false }
        guard now < deadline else { state = .timedOut; return false }
        state = .completed(value)
        return true
    }

    func poll(now: TimeInterval = ProcessInfo.processInfo.systemUptime) -> LaunchState<Value> {
        lock.lock(); defer { lock.unlock() }
        if case .pending = state, now >= deadline { state = .timedOut }
        return state
    }
}
