import Foundation
import ScreenCaptureKit

private struct CaptureErrorTestFailure: Error {
    let message: String
}

private func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw CaptureErrorTestFailure(message: message) }
}

private func sdkFailure(_ phase: String, domain: String = SCStreamErrorDomain,
                        code: Int = SCStreamError.userDeclined.rawValue) -> LockCaptureFailure {
    let error = NSError(domain: domain, code: code,
                        userInfo: [NSLocalizedDescriptionKey: "permission denied: private window text"])
    return captureError(phase, error)
}

@main private struct LockCaptureErrorTests {
    static func main() throws {
        let phases = ["enumeration", "stream output", "stream startup", "stream", "stream stop", "screenshot"]
        var permission: [String: Any] = [:]
        for phase in phases {
            let failure = sdkFailure(phase)
            let sample = failure.sample()
            try require(sample.status == "permission_denied", "missing permission status: \(phase)")
            try require(sample.sequence == nil && sample.frameCount == 0, "invalid error observation")
            try require(sample.error == "\(phase) failed (code -3801)", "unsanitized capture error")
            permission[phase] = sample.record
        }
        let otherDomain = sdkFailure("screenshot", domain: "unrelated.domain").sample()
        let otherCode = sdkFailure("screenshot", code: SCStreamError.failedToStart.rawValue).sample()
        let localFailure = LockCaptureFailure(description: "permission denied text is not classification").sample()
        try require([otherDomain, otherCode, localFailure].allSatisfy { $0.status == "error" },
                    "permission guessed from code or words")
        let output = RetainedCaptureOutput()
        output.markStopped(failure: sdkFailure("stream"))
        let terminal = output.snapshot()
        try require(terminal.status == "permission_denied", "delegate terminal classification lost")
        output.markStopped()
        let stopped = output.snapshot()
        try require(stopped.status == "stopped" && stopped.error == nil, "confirmed stop is not a failure")
        output.markStopped(failure: sdkFailure("stream stop"))
        let stopFailure = output.snapshot()
        try require(stopFailure.status == "permission_denied" && stopFailure.error?.isEmpty == false,
                    "failed stop lacks shutdown evidence")
        output.markStopped(failure: sdkFailure("stream", code: SCStreamError.failedToStart.rawValue))
        try require(output.snapshot().status == "stopped", "generic terminal status changed")
        let records: [String: Any] = ["permission": permission, "other_domain": otherDomain.record,
            "other_code": otherCode.record, "local_error": localFailure.record,
            "terminal_permission": terminal.record, "confirmed_stop": stopped.record,
            "stop_permission": stopFailure.record]
        let data = try JSONSerialization.data(withJSONObject: records, options: [.sortedKeys])
        FileHandle.standardOutput.write(data)
    }
}
