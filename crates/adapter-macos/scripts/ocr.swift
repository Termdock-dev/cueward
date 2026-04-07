import Foundation
import Vision
import AppKit
import PDFKit

struct OcrResult: Codable {
    let text: String
    let confidence: Float
}

func ocrImage(_ path: String) -> [OcrResult] {
    let url = URL(fileURLWithPath: path)

    // Check if PDF
    if path.lowercased().hasSuffix(".pdf") {
        return ocrPdf(url)
    }

    guard let ciImage = CIImage(contentsOf: url) else {
        fputs("error: could not load image at \(path)\n", stderr)
        return []
    }

    return recognizeText(in: ciImage)
}

func ocrPdf(_ url: URL) -> [OcrResult] {
    guard let document = PDFDocument(url: url) else {
        fputs("error: could not load PDF at \(url.path)\n", stderr)
        return []
    }

    var allResults: [OcrResult] = []

    for i in 0..<document.pageCount {
        guard let page = document.page(at: i) else { continue }

        // Try native text first
        if let rawText = page.string, !rawText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            allResults.append(OcrResult(text: rawText, confidence: 1.0))
            continue
        }

        // Fall back to Vision OCR
        let pageRect = page.bounds(for: .mediaBox)
        let image = NSImage(size: pageRect.size)
        image.lockFocus()
        if let context = NSGraphicsContext.current?.cgContext {
            context.setFillColor(NSColor.white.cgColor)
            context.fill(pageRect)
            page.draw(with: .mediaBox, to: context)
        }
        image.unlockFocus()

        guard let tiffData = image.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffData),
              let cgImage = bitmap.cgImage else { continue }

        let ciImage = CIImage(cgImage: cgImage)
        allResults.append(contentsOf: recognizeText(in: ciImage))
    }

    return allResults
}

func recognizeText(in ciImage: CIImage) -> [OcrResult] {
    var results: [OcrResult] = []

    let handler = VNImageRequestHandler(ciImage: ciImage, options: [:])
    let request = VNRecognizeTextRequest { request, _ in
        guard let observations = request.results as? [VNRecognizedTextObservation] else { return }
        for observation in observations {
            guard let candidate = observation.topCandidates(1).first else { continue }
            results.append(OcrResult(text: candidate.string, confidence: candidate.confidence))
        }
    }

    request.recognitionLanguages = ["zh-Hant", "zh-Hans", "en-US", "ja"]
    request.usesLanguageCorrection = true

    try? handler.perform([request])
    return results
}

// Main
let args = CommandLine.arguments
guard args.count > 1 else {
    fputs("usage: ocr.swift <image_or_pdf_path>\n", stderr)
    exit(1)
}

let results = ocrImage(args[1])
let encoder = JSONEncoder()
encoder.outputFormatting = .prettyPrinted
do {
    let data = try encoder.encode(results)
    if let json = String(data: data, encoding: .utf8) {
        print(json)
    } else {
        fputs("error: failed to encode JSON as UTF-8\n", stderr)
        exit(1)
    }
} catch {
    fputs("error: JSON encoding failed: \(error)\n", stderr)
    exit(1)
}
