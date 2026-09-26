import Foundation

func require(_ result: Bool, _ message: String) {
    if !result { fputs(message + "\n", stderr); exit(1) }
}
let completed = LaunchCompletion<Int>(deadline: 100)
require(completed.complete(7, now: 99), "completion before deadline lost")
require(!completed.complete(9, now: 99), "duplicate completion accepted")
if case .completed(7) = completed.poll(now: 101) {} else { fatalError("timeout replaced completed value") }
let expired = LaunchCompletion<Int>(deadline: 100)
if case .timedOut = expired.poll(now: 100) {} else { fatalError("deadline did not expire") }
require(!expired.complete(7, now: 99), "completion reopened timed-out state")
let late = LaunchCompletion<Int>(deadline: 100)
require(!late.complete(7, now: 100), "late callback accepted")
if case .timedOut = late.poll(now: 100) {} else { fatalError("late completion did not record timeout") }

for _ in 0..<100 {
    let gate = LaunchCompletion<Int>(deadline: 100)
    let lock = NSLock()
    var winners: [Int] = []
    DispatchQueue.concurrentPerform(iterations: 32) { index in
        if index.isMultiple(of: 2) {
            if gate.complete(index, now: 99) { lock.lock(); winners.append(index); lock.unlock() }
        } else { _ = gate.poll(now: 100) }
    }
    switch gate.poll(now: 101) {
    case .pending: fatalError("gate stayed pending")
    case .completed(let value): require(winners == [value], "completion winners disagree")
    case .timedOut: require(winners.isEmpty, "completion and timeout both won")
    }
}
print("passed")
