import CoreMedia
import CoreVideo
import Foundation
import ScreenCaptureKit

struct LockCaptureSample: Sendable {
    let status: String
    let sequence: Int?
    let frameCount: Int
    let uptime: Double
    let error: String?

    var record: [String: Any] {
        ["status": status, "sequence": sequence.map { $0 as Any } ?? NSNull(),
         "frameCount": frameCount, "uptime": uptime,
         "error": error.map { $0 as Any } ?? NSNull()]
    }
}

private struct LockCaptureFailure: Error, Sendable, CustomStringConvertible, LocalizedError {
    let description: String
    var errorDescription: String? { description }
}

private func captureError(_ phase: String, _ error: any Error) -> String {
    // SDK localized descriptions can contain external data. Record only a fixed phase/code.
    "\(phase) failed (code \((error as NSError).code))"
}

private func captureSample(_ status: String, sequence: Int? = nil,
                           count: Int = 0, error: String? = nil) -> LockCaptureSample {
    LockCaptureSample(status: status, sequence: sequence, frameCount: count,
                      uptime: ProcessInfo.processInfo.systemUptime, error: error)
}

/// Callback APIs are not cancellable. Resume once and abandon late callbacks, never await them.
private final class CaptureDeadline<Value: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Value, Never>?

    init(_ continuation: CheckedContinuation<Value, Never>, timeout: @escaping @Sendable () -> Value) {
        self.continuation = continuation
        DispatchQueue.global().asyncAfter(deadline: .now() + 3) { self.finish(timeout()) }
    }

    @discardableResult func finish(_ value: Value) -> Bool {
        lock.lock()
        let pending = continuation
        continuation = nil
        lock.unlock()
        pending?.resume(returning: value)
        return pending != nil
    }

    var isPending: Bool {
        lock.lock()
        defer { lock.unlock() }
        return continuation != nil
    }
}

/// Transferred once from the enumeration callback, then used only on MainActor.
private final class OwnedCaptureFilter: @unchecked Sendable {
    let filter: SCContentFilter
    init(_ filter: SCContentFilter) { self.filter = filter }
}

private func captureConfiguration(_ filter: SCContentFilter) throws -> SCStreamConfiguration {
    let size = filter.contentRect.size
    guard size.width.isFinite, size.height.isFinite,
          size.width >= 1, size.height >= 1, size.width <= 1024, size.height <= 1024 else {
        throw LockCaptureFailure(description: "owned window has unsupported capture dimensions")
    }
    let config = SCStreamConfiguration()
    config.width = Int(size.width.rounded(.up))
    config.height = Int(size.height.rounded(.up))
    config.pixelFormat = kCVPixelFormatType_32BGRA
    config.minimumFrameInterval = CMTime(value: 1, timescale: 10)
    config.queueDepth = 3
    config.showsCursor = false
    config.showMouseClicks = false
    config.capturesAudio = false
    config.captureMicrophone = false
    config.includeChildWindows = false
    config.ignoreShadowsSingleWindow = true
    config.ignoreGlobalClipSingleWindow = true
    config.captureResolution = .nominal
    config.captureDynamicRange = .SDR
    config.scalesToFit = false
    return config
}

private func ownedFilter(_ content: SCShareableContent?, pid: Int32,
                         windowID: UInt32) -> SCContentFilter? {
    guard let window = content?.windows.first(where: {
        $0.windowID == windowID && $0.owningApplication?.processID == pid
    }) else { return nil }
    let filter = SCContentFilter(desktopIndependentWindow: window)
    filter.includeMenuBar = false
    return filter
}

private func freshOwnedFilter(pid: Int32, windowID: UInt32)
    async -> Result<OwnedCaptureFilter, LockCaptureFailure> {
    await withCheckedContinuation { continuation in
        let gate = CaptureDeadline(continuation, timeout: { .failure(LockCaptureFailure(
            description: "owned window enumeration timed out")) })
        SCShareableContent.getExcludingDesktopWindows(true, onScreenWindowsOnly: false) { content, error in
            guard gate.isPending else { return }
            if let error {
                gate.finish(.failure(LockCaptureFailure(description: captureError("enumeration", error))))
            } else if let filter = ownedFilter(content, pid: pid, windowID: windowID) {
                gate.finish(.success(OwnedCaptureFilter(filter)))
            } else {
                gate.finish(.failure(LockCaptureFailure(description: "owned PID/window binding unavailable")))
            }
        }
    }
}

private func decodeCaptureBuffer(_ buffer: CMSampleBuffer, count: Int) -> LockCaptureSample {
    guard CMSampleBufferIsValid(buffer), let pixels = CMSampleBufferGetImageBuffer(buffer),
          CVPixelBufferGetPixelFormatType(pixels) == kCVPixelFormatType_32BGRA,
          CVPixelBufferLockBaseAddress(pixels, .readOnly) == kCVReturnSuccess else {
        return captureSample("undecodable", count: count, error: "BGRA image buffer unavailable")
    }
    defer { CVPixelBufferUnlockBaseAddress(pixels, .readOnly) }
    guard let base = CVPixelBufferGetBaseAddress(pixels) else {
        return captureSample("undecodable", count: count, error: "BGRA base address unavailable")
    }
    let height = CVPixelBufferGetHeight(pixels)
    let rowBytes = CVPixelBufferGetBytesPerRow(pixels)
    let decoded = decodeLockPattern(width: CVPixelBufferGetWidth(pixels), height: height,
        bytesPerRow: rowBytes, bgra: UnsafeRawBufferPointer(start: base, count: rowBytes * height))
    return captureSample(decoded.status, sequence: decoded.sequence, count: count)
}

private func streamFrameStatus(_ buffer: CMSampleBuffer) -> SCFrameStatus? {
    guard let attachments = CMSampleBufferGetSampleAttachmentsArray(buffer,
        createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
          let number = attachments.first?[.status] as? NSNumber else { return nil }
    return SCFrameStatus(rawValue: number.intValue)
}

/// Framework callbacks stay off MainActor; only decoded scalars enter locked state.
private final class RetainedCaptureOutput: NSObject, SCStreamOutput, SCStreamDelegate, @unchecked Sendable {
    private let lock = NSLock()
    private var current = captureSample("no_frame")
    private var stopped = false

    func snapshot() -> LockCaptureSample {
        lock.lock()
        defer { lock.unlock() }
        return current
    }

    func markStopped(error: String? = nil) {
        lock.lock()
        defer { lock.unlock() }
        stopped = true
        current = captureSample("stopped", count: current.frameCount, error: error)
    }

    func stream(_ stream: SCStream, didStopWithError error: any Error) {
        markStopped(error: captureError("stream", error))
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer buffer: CMSampleBuffer,
                of type: SCStreamOutputType) {
        guard type == .screen, let status = streamFrameStatus(buffer) else { return }
        if status == .complete {
            guard let count = nextFrameCount() else { return }
            let decoded = decodeCaptureBuffer(buffer, count: count)
            publish(decoded)
        } else {
            publishFrameStatus(status)
        }
    }

    private func nextFrameCount() -> Int? {
        lock.lock()
        defer { lock.unlock() }
        return stopped ? nil : current.frameCount + 1
    }

    private func publish(_ sample: LockCaptureSample) {
        lock.lock()
        defer { lock.unlock() }
        guard !stopped else { return }
        current = sample
    }

    private func publishFrameStatus(_ status: SCFrameStatus) {
        lock.lock()
        defer { lock.unlock() }
        guard !stopped else { return }
        if status == .blank {
            current = captureSample("blank", count: current.frameCount)
        } else if status == .stopped {
            stopped = true
            current = captureSample("stopped", count: current.frameCount)
        }
    }
}

/// Retain the startup filter/stream unchanged. Every screenshot resolves a fresh exact window.
@MainActor final class RetainedLockCapture {
    private let pid: Int32
    private let windowID: UInt32
    private let filter: SCContentFilter
    private let stream: SCStream
    private let output: RetainedCaptureOutput
    private let callbackQueue = DispatchQueue(label: "dev.cueward.lock-capture")
    private var stopped = false

    private init(pid: Int32, windowID: UInt32, filter: SCContentFilter) throws {
        self.pid = pid
        self.windowID = windowID
        self.filter = filter
        output = RetainedCaptureOutput()
        stream = SCStream(filter: filter, configuration: try captureConfiguration(filter), delegate: output)
        do { try stream.addStreamOutput(output, type: .screen, sampleHandlerQueue: callbackQueue) }
        catch { throw LockCaptureFailure(description: captureError("stream output", error)) }
    }

    static func start(pid: Int32, windowID: UInt32) async throws -> RetainedLockCapture {
        guard pid > 0, windowID > 0 else {
            throw LockCaptureFailure(description: "positive owned PID/window ID required")
        }
        let resolved = try await freshOwnedFilter(pid: pid, windowID: windowID).get()
        let capture = try RetainedLockCapture(pid: pid, windowID: windowID, filter: resolved.filter)
        let result = await capture.startStream()
        if case .failure(let error) = result {
            await capture.stop()
            throw error
        }
        return capture
    }

    func snapshot() -> LockCaptureSample { output.snapshot() }

    func screenshot() async -> LockCaptureSample {
        guard !stopped else { return captureSample("stopped") }
        let targetPID = pid
        let targetWindow = windowID
        return await withCheckedContinuation { continuation in
            let gate = CaptureDeadline(continuation, timeout: { captureSample("timeout", error: "screenshot timed out") })
            SCShareableContent.getExcludingDesktopWindows(true, onScreenWindowsOnly: false) { content, error in
                guard gate.isPending else { return }
                if let error { gate.finish(captureSample("error", error: captureError("enumeration", error))); return }
                guard let filter = ownedFilter(content, pid: targetPID, windowID: targetWindow) else {
                    gate.finish(captureSample("no_frame", error: "owned PID/window binding unavailable")); return
                }
                startFreshScreenshot(filter: filter, gate: gate)
            }
        }
    }

    func stop() async {
        guard !stopped else { return }
        stopped = true
        output.markStopped()
        await forceStop()
    }

    private func startStream() async -> Result<Void, LockCaptureFailure> {
        await withCheckedContinuation { continuation in
            let gate = CaptureDeadline(continuation, timeout: { .failure(LockCaptureFailure(
                description: "stream startup timed out")) })
            stream.startCapture { error in
                let result: Result<Void, LockCaptureFailure> = error.map {
                    .failure(LockCaptureFailure(description: captureError("stream startup", $0)))
                } ?? .success(())
                if !gate.finish(result), error == nil {
                    Task { @MainActor in await self.forceStop() }
                }
            }
        }
    }

    private func forceStop() async {
        // Also called after a late successful start, even if an earlier stop was requested.
        let result: Result<Void, LockCaptureFailure> = await withCheckedContinuation { continuation in
            let gate = CaptureDeadline(continuation, timeout: { .failure(LockCaptureFailure(
                description: "stream stop timed out; underlying stop unconfirmed")) })
            stream.stopCapture { error in
                gate.finish(error.map { .failure(LockCaptureFailure(
                    description: captureError("stream stop", $0))) } ?? .success(()))
            }
        }
        if case .failure(let error) = result { output.markStopped(error: error.description) }
    }
}

private func startFreshScreenshot(filter: SCContentFilter, gate: CaptureDeadline<LockCaptureSample>) {
    guard gate.isPending else { return }
    do {
        let config = try captureConfiguration(filter)
        guard gate.isPending else { return }
        SCScreenshotManager.captureSampleBuffer(contentFilter: filter, configuration: config) { buffer, error in
            guard gate.isPending else { return }
            if let error { gate.finish(captureSample("error", error: captureError("screenshot", error))); return }
            guard let buffer else { gate.finish(captureSample("no_frame")); return }
            gate.finish(decodeCaptureBuffer(buffer, count: 1))
        }
    } catch {
        gate.finish(captureSample("error", error: "owned window has unsupported capture dimensions"))
    }
}
