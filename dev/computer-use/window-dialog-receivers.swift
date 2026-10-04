import Foundation

struct DialogReceiverObservation {
    let ref: String
    let pid: Int32
    let nativeWindowID: UInt32?
}

struct DialogReceiverBindingError: Error, CustomStringConvertible {
    let description: String
    init(_ message: String) { description = message }
}

private func validDialogReceiverRef(_ value: String) -> Bool {
    value.range(of: "^w[0-9]+(?:\\.[0-9]+){0,12}$", options: .regularExpression)
        == value.startIndex..<value.endIndex
}

// This boundary is pure: the AX reader supplies observations and process liveness.
func boundDialogSheetReceivers(_ observations: [DialogReceiverObservation], hostPID: Int32,
                              root: String, panelID: UInt32, isAlive: (Int32) -> Bool) throws -> [Int32] {
    guard hostPID > 0, panelID > 0, validDialogReceiverRef(root) else {
        throw DialogReceiverBindingError("invalid owned sheet receiver context")
    }
    let scoped = observations.filter { $0.ref == root || $0.ref.hasPrefix(root + ".") }
    guard scoped.filter({ $0.ref == root }).count == 1 else {
        throw DialogReceiverBindingError("selected sheet receiver must be present and unique")
    }
    var references = Set<String>()
    for observation in scoped {
        guard validDialogReceiverRef(observation.ref), references.insert(observation.ref).inserted,
              observation.pid > 0, isAlive(observation.pid) else {
            throw DialogReceiverBindingError("invalid or unavailable sheet receiver at \(observation.ref)")
        }
        // Host context is already within the retained host process scope. Every
        // foreign descendant must bind directly to this panel, never be dropped.
        if observation.ref == root || observation.pid != hostPID {
            guard observation.nativeWindowID == panelID else {
                throw DialogReceiverBindingError("unbound sheet receiver at \(observation.ref)")
            }
        }
    }
    return Set(scoped.filter { $0.nativeWindowID == panelID }.map { $0.pid }).sorted()
}
