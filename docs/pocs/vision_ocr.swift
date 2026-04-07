import Foundation
import Vision
import AppKit

func recognizeText(in imagePath: String) {
    let imageURL = URL(fileURLWithPath: imagePath)
    guard let ciImage = CIImage(contentsOf: imageURL) else {
        print("Error: Could not load image at \(imagePath)")
        return
    }

    let requestHandler = VNImageRequestHandler(ciImage: ciImage, options: [:])
    let request = VNRecognizeTextRequest { (request, error) in
        guard let observations = request.results as? [VNRecognizedTextObservation] else { return }
        
        var results: [[String: Any]] = []
        for observation in observations {
            guard let candidate = observation.topCandidates(1).first else { continue }
            let item: [String: Any] = [
                "text": candidate.string,
                "confidence": candidate.confidence,
                "box": [
                    "x": observation.boundingBox.origin.x,
                    "y": observation.boundingBox.origin.y,
                    "width": observation.boundingBox.size.width,
                    "height": observation.boundingBox.size.height
                ]
            ]
            results.append(item)
        }
        
        if let jsonData = try? JSONSerialization.data(withJSONObject: results, options: .prettyPrinted),
           let jsonString = String(data: jsonData, encoding: .utf8) {
            print(jsonString)
        }
    }

    request.recognitionLanguages = ["zh-Hant", "en-US"]
    request.usesLanguageCorrection = true

    try? requestHandler.perform([request])
}

// 指令介面：傳入圖片路徑
let arguments = CommandLine.arguments
if arguments.count > 1 {
    recognizeText(in: arguments[1])
} else {
    print("Usage: swift vision_ocr.swift <image_path>")
}
