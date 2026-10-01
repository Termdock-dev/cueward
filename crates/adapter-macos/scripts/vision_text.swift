import Foundation
import Vision
import CoreImage

struct OcrResult: Codable {
    let text: String
    let confidence: Float
}

// Shared Vision implementation; callers decide whether an error is fatal or partial.
func recognizeText(in ciImage: CIImage) throws -> [OcrResult] {
    let request = VNRecognizeTextRequest()
    request.recognitionLanguages = ["zh-Hant", "zh-Hans", "en-US", "ja"]
    request.usesLanguageCorrection = true
    try VNImageRequestHandler(ciImage: ciImage, options: [:]).perform([request])
    return (request.results ?? []).compactMap { observation in
        guard let candidate = observation.topCandidates(1).first else { return nil }
        return OcrResult(text: candidate.string, confidence: candidate.confidence)
    }
}
