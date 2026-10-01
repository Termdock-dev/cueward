func pdfPreview(_ request: NativeRequest, budget: Budget) throws -> [String: Any] {
    let url = URL(fileURLWithPath: request.input)
    let header = try FileHandle(forReadingFrom: url)
    defer { try? header.close() }
    let signature = try header.read(upToCount: 1024) ?? Data()
    guard signature.range(of: Data("%PDF-".utf8)) != nil else { throw fail("unsupported_type", "source has no PDF signature") }
    guard let document = PDFDocument(url: url) else { throw fail("corrupt_data", "PDFKit could not decode this PDF") }
    guard !document.isLocked else { throw fail("encrypted", "PDF is locked; password input is not supported") }
    guard document.allowsCopying else { throw fail("permission_denied", "PDF disallows content extraction") }
    let options = request.options
    let total = document.pageCount
    guard (total == 0 && options.start_page == 1) || options.start_page <= total else {
        throw fail("invalid_options", "start-page exceeds PDF page count")
    }
    let last = min(total, options.start_page + options.page_count - 1)
    var pages: [[String: Any]] = []
    if last >= options.start_page {
        for number in options.start_page...last {
            pages.append(pdfPage(document.page(at: number - 1), number: number, request: request, budget: budget))
        }
    }
    return ["total_pages": total, "pdf_encrypted": document.isEncrypted,
            "pages": pages, "selection_truncated": options.start_page > 1 || last < total]
}
func pdfPage(_ page: PDFPage?, number: Int, request: NativeRequest, budget: Budget) -> [String: Any] {
    var result = pageResult(number)
    result["text_source"] = "native_pdf"
    guard let page else {
        let error = fieldError(fail("corrupt_data", "PDF page could not be loaded"))
        result["text"] = error
        result["preview"] = error
        return result
    }
    let native = page.string
    let needsOcr = request.options.ocr && (native?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ?? true)
    if let native, !needsOcr { setText(native, source: "native_pdf", confidence: nil, budget: budget, result: &result) }
    else { result["text"] = skipped("unavailable") }
    if request.options.render || needsOcr {
        do {
            let image = try renderPdf(page, dimension: budget.dimension)
            if needsOcr { setOcr(image, budget: budget, result: &result) }
            if request.options.render { setPreview(image, page: number, directory: request.directory, budget: budget, result: &result) }
        } catch {
            if needsOcr { result["text_source"] = "vision_ocr"; result["text"] = fieldError(error) }
            if request.options.render { result["preview"] = fieldError(error) }
        }
    }
    return result
}
