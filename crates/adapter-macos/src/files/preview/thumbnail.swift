final class ThumbnailReply: @unchecked Sendable {
    private let lock = NSLock()
    private var result: Result<CGImage, Error>?
    func receive(_ representation: QLThumbnailRepresentation?, _ error: Error?) {
        lock.lock(); defer { lock.unlock() }
        if let error { result = .failure(error) }
        else if let representation, representation.type == .thumbnail { result = .success(representation.cgImage) }
        else { result = .failure(fail("unavailable", "Quick Look returned no content thumbnail; icons are not previews")) }
    }
    func take() -> Result<CGImage, Error>? { lock.lock(); defer { lock.unlock() }; return result }
}
func thumbnailPreview(_ request: NativeRequest, budget: Budget) -> [String: Any] {
    let size = CGSize(width: budget.dimension, height: budget.dimension)
    let native = QLThumbnailGenerator.Request(fileAt: URL(fileURLWithPath: request.input), size: size,
        scale: 1, representationTypes: .thumbnail)
    native.iconMode = false
    let reply = ThumbnailReply()
    QLThumbnailGenerator.shared.generateBestRepresentation(for: native) { representation, error in reply.receive(representation, error) }
    let deadline = Date().addingTimeInterval(25)
    while reply.take() == nil && Date() < deadline {
        _ = RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.01))
    }
    var result = pageResult(1)
    switch reply.take() {
    case .success(let image): setPreview(image, page: 1, directory: request.directory, budget: budget, result: &result)
    case .failure(let error): result["preview"] = fieldError(error)
    case nil:
        QLThumbnailGenerator.shared.cancel(native)
        result["preview"] = fieldError(fail("timeout", "Quick Look did not finish before its native deadline"))
    }
    return ["pages": [result], "selection_truncated": true]
}
