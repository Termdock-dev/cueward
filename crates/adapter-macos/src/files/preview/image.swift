func imagePreview(_ request: NativeRequest, budget: Budget) throws -> [String: Any] {
    let url = URL(fileURLWithPath: request.input)
    guard let source = CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary),
          let type = CGImageSourceGetType(source) else { throw fail("unsupported_type", "ImageIO does not recognize this source") }
    guard CGImageSourceGetStatus(source) == .statusComplete,
          CGImageSourceGetCount(source) > 0,
          let info = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
          let width = info[kCGImagePropertyPixelWidth] as? Int,
          let height = info[kCGImagePropertyPixelHeight] as? Int, width > 0, height > 0 else {
        throw fail("corrupt_data", "image data or dimensions are incomplete")
    }
    var result = pageResult(1)
    if request.options.render || request.options.ocr {
        let options = [kCGImageSourceCreateThumbnailFromImageAlways: true,
                       kCGImageSourceCreateThumbnailWithTransform: true,
                       kCGImageSourceThumbnailMaxPixelSize: budget.dimension,
                       kCGImageSourceShouldCacheImmediately: true] as CFDictionary
        if let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options) {
            if request.options.ocr { setOcr(image, budget: budget, result: &result) }
            if request.options.render { setPreview(image, page: 1, directory: request.directory, budget: budget, result: &result) }
        } else {
            let error = fieldError(fail("corrupt_data", "ImageIO could not render the selected frame"))
            if request.options.ocr { result["text_source"] = "vision_ocr"; result["text"] = error }
            if request.options.render { result["preview"] = error }
        }
    }
    let imageInfo: [String: Any] = ["content_type": type as String, "width": width, "height": height,
        "frame_count": CGImageSourceGetCount(source), "orientation": info[kCGImagePropertyOrientation] ?? NSNull()]
    return ["image": imageInfo, "pages": [result], "selection_truncated": CGImageSourceGetCount(source) > 1]
}
