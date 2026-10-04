import Foundation

struct DialogReceiverObservation {
    let ref: String
    let pid: Int32
    let nativeWindowID: UInt32?
}

// This boundary is pure: the AX reader supplies observations and process liveness.
func boundDialogSheetReceivers(_ observations: [DialogReceiverObservation], hostPID: Int32,
                              root: String, panelID: UInt32, isAlive: (Int32) -> Bool) throws -> [Int32] {
    Set(observations.filter { ($0.ref == root || $0.ref.hasPrefix(root + "."))
        && $0.nativeWindowID == panelID && $0.pid > 0 && isAlive($0.pid) }.map { $0.pid }).sorted()
}
