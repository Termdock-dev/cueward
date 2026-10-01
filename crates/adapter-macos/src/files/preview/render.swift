func png(_ image: CGImage, page: Int, directory: String, budget: Budget) throws -> [String: Any] {
    guard image.width > 0 && image.height > 0 && image.width <= budget.dimension && image.height <= budget.dimension else {
        throw fail("scan_limit", "native preview exceeds pixel bounds")
    }
    let data = NSMutableData()
    guard let destination = CGImageDestinationCreateWithData(data, "public.png" as CFString, 1, nil) else {
        throw fail("unavailable", "cannot create PNG encoder")
    }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else { throw fail("corrupt_data", "PNG encoding failed") }
    guard data.length <= budget.images else { throw fail("scan_limit", "aggregate preview byte limit exceeded") }
    let name = "page-\(page).png"
    try (data as Data).write(to: URL(fileURLWithPath: directory).appendingPathComponent(name), options: .withoutOverwriting)
    budget.images -= data.length
    return ["path": name, "format": "png", "width": image.width, "height": image.height, "bytes": data.length, "sha256": ""]
}
func renderPdf(_ page: PDFPage, dimension: Int) throws -> CGImage {
    let rect = page.bounds(for: .mediaBox)
    guard rect.width.isFinite && rect.height.isFinite && rect.width > 0 && rect.height > 0 else {
        throw fail("corrupt_data", "PDF page has invalid dimensions")
    }
    page.displaysAnnotations = true
    let thumbnail = page.thumbnail(of: CGSize(width: dimension, height: dimension), for: .mediaBox)
    guard let native = thumbnail.cgImage(forProposedRect: nil, context: nil, hints: nil), native.width > 0, native.height > 0 else {
        throw fail("unavailable", "PDFKit thumbnail produced no image")
    }
    let ratio = min(1, min(CGFloat(dimension) / CGFloat(native.width), CGFloat(dimension) / CGFloat(native.height)))
    let width = max(1, Int(floor(CGFloat(native.width) * ratio)))
    let height = max(1, Int(floor(CGFloat(native.height) * ratio)))
    guard let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                                  bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
        throw fail("unavailable", "cannot create bounded PDF rendering context")
    }
    context.setFillColor(CGColor(gray: 1, alpha: 1))
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    // PDFKit handles media origin, rotation and annotations; resample any HiDPI representation.
    context.draw(native, in: CGRect(x: 0, y: 0, width: width, height: height))
    guard let image = context.makeImage() else { throw fail("unavailable", "PDF rendering produced no image") }
    return image
}
func setPreview(_ image: CGImage, page: Int, directory: String, budget: Budget, result: inout [String: Any]) {
    do { result["preview"] = available(try png(image, page: page, directory: directory, budget: budget)) }
    catch { result["preview"] = fieldError(error) }
}
