import Foundation
import PDFKit
import ImageIO
import AppKit
import QuickLookThumbnailing

struct PreviewOptions: Decodable {
    var kind: String
    var start_page: Int
    var page_count: Int
    var ocr: Bool
    var render: Bool
    var max_input_bytes: Int
    var max_text_bytes: Int
    var max_dimension: Int
    var max_preview_bytes: Int
}
struct NativeRequest: Decodable {
    var input: String
    var directory: String
    var options: PreviewOptions
}
struct Failure: Error, Codable {
    var code: String
    var message: String
}
func fail(_ code: String, _ message: String) -> Failure { Failure(code: code, message: message) }
func fieldError(_ error: Error) -> [String: Any] {
    let native: NSError
    if let failure = error as? Failure {
        native = NSError(domain: "cueward.files.preview", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(failure.code): \(failure.message)"])
    } else { native = error as NSError }
    return ["status": "error", "error": ["domain": native.domain, "code": native.code, "message": native.localizedDescription]]
}
func available(_ value: Any) -> [String: Any] { ["status": "available", "value": value] }
func skipped(_ reason: String) -> [String: Any] { ["status": reason] }

final class Budget {
    var text: Int
    var images: Int
    let dimension: Int
    init(_ options: PreviewOptions) {
        text = options.max_text_bytes
        images = options.max_preview_bytes
        dimension = options.max_dimension
    }
    func textValue(_ text: String) -> (String, Bool) {
        // Prefix must end at a Unicode scalar boundary, not an arbitrary byte.
        let clipped = text.utf8.count > self.text
        var bytes = Array(text.utf8.prefix(self.text))
        while String(bytes: bytes, encoding: .utf8) == nil { bytes.removeLast() }
        self.text -= bytes.count
        return (String(bytes: bytes, encoding: .utf8) ?? "", clipped)
    }
}
func pageResult(_ page: Int) -> [String: Any] {
    ["page": page, "text": skipped("not_applicable"), "text_source": "not_requested",
     "confidence": NSNull(), "text_truncated": false, "preview": skipped("not_applicable")]
}
func setText(_ value: String, source: String, confidence: Float?, budget: Budget, result: inout [String: Any]) {
    let (text, truncated) = budget.textValue(value)
    result["text"] = available(text)
    result["text_source"] = source
    result["confidence"] = confidence.map { $0 as Any } ?? NSNull()
    result["text_truncated"] = truncated
}
func setOcr(_ image: CGImage, budget: Budget, result: inout [String: Any]) {
    result["text_source"] = "vision_ocr"
    do {
        let lines = try recognizeText(in: CIImage(cgImage: image))
        let confidence = lines.isEmpty ? nil : lines.map(\.confidence).reduce(0, +) / Float(lines.count)
        setText(lines.map(\.text).joined(separator: "\n"), source: "vision_ocr", confidence: confidence, budget: budget, result: &result)
    } catch { result["text"] = fieldError(error) }
}
