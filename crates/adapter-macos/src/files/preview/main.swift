@_silgen_name("setiopolicy_np") private func setPreviewPolicy(_ policy: Int32, _ scope: Int32, _ value: Int32) -> Int32
func performPreview(_ request: NativeRequest) throws -> [String: Any] {
    guard setPreviewPolicy(3, 1, 1) == 0 else { throw fail("unavailable", "cannot deny dataless materialization in native helper") }
    let budget = Budget(request.options)
    switch request.options.kind {
    case "pdf": return try pdfPreview(request, budget: budget)
    case "image": return try imagePreview(request, budget: budget)
    case "thumbnail": return thumbnailPreview(request, budget: budget)
    default: throw fail("invalid_options", "unknown preview kind")
    }
}
do {
    let input = FileHandle.standardInput.readDataToEndOfFile()
    let request = try JSONDecoder().decode(NativeRequest.self, from: input)
    let result = try performPreview(request)
    let payload = try JSONSerialization.data(withJSONObject: ["Ok": result], options: [.sortedKeys])
    guard payload.count <= 8 * 1024 * 1024 else { throw fail("scan_limit", "native preview response exceeds 8 MiB") }
    FileHandle.standardOutput.write(payload)
} catch {
    let value = error as? Failure ?? fail("unavailable", (error as NSError).localizedDescription)
    do { FileHandle.standardOutput.write(try JSONEncoder().encode(["Err": value])) }
    catch { fputs("preview result encoding failed\n", stderr) }
    exit(1)
}
