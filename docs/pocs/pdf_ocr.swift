import Foundation
import PDFKit
import Vision
import AppKit

func processPDF(at path: String) {
    let url = URL(fileURLWithPath: path)
    guard let document = PDFDocument(url: url) else {
        print("Error: Could not load PDF at \(path)")
        return
    }
    
    let pageCount = document.pageCount
    print("📄 載入 PDF: \(url.lastPathComponent) (共 \(pageCount) 頁)")
    
    for i in 0..<pageCount {
        guard let page = document.page(at: i) else { continue }
        print("\n--- 正在處理第 \(i + 1) 頁 ---")
        
        // 策略 1: 嘗試直接提取原生文字 (Text-based PDF)
        if let rawText = page.string, !rawText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            print("✅ 偵測到原生文字層 (快速提取):")
            // 預覽前 100 個字元
            let preview = String(rawText.prefix(100)).replacingOccurrences(of: "\n", with: " ")
            print("\(preview)...")
            continue
        }
        
        // 策略 2: 如果沒有原生文字，啟動 Vision OCR (Scanned PDF)
        print("⚠️ 無原生文字，啟動 Vision OCR 圖片辨識...")
        
        // 將 PDF 頁面渲染為圖片
        let pageRect = page.bounds(for: .mediaBox)
        let image = NSImage(size: pageRect.size)
        image.lockFocus()
        if let context = NSGraphicsContext.current?.cgContext {
            // 填充白色背景 (避免透明背景影響 OCR)
            context.setFillColor(NSColor.white.cgColor)
            context.fill(pageRect)
            page.draw(with: .mediaBox, to: context)
        }
        image.unlockFocus()
        
        // 將 NSImage 轉換為 CGImage 供 Vision 使用
        guard let tiffData = image.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffData),
              let cgImage = bitmap.cgImage else {
            print("❌ 圖片渲染失敗")
            continue
        }
        
        // 執行 OCR
        let requestHandler = VNImageRequestHandler(cgImage: cgImage, options: [:])
        let request = VNRecognizeTextRequest { (request, error) in
            guard let observations = request.results as? [VNRecognizedTextObservation] else { return }
            var recognizedText = ""
            for observation in observations {
                guard let candidate = observation.topCandidates(1).first else { continue }
                recognizedText += candidate.string + " "
            }
            // 預覽辨識結果
            let preview = String(recognizedText.prefix(100)).replacingOccurrences(of: "\n", with: " ")
            print("👁️ OCR 辨識結果:")
            print("\(preview)...")
        }
        
        request.recognitionLanguages = ["zh-Hant", "en-US"]
        request.usesLanguageCorrection = true
        
        do {
            try requestHandler.perform([request])
        } catch {
            print("❌ OCR 執行失敗: \(error)")
        }
    }
}

// 指令介面：傳入 PDF 路徑
let arguments = CommandLine.arguments
if arguments.count > 1 {
    processPDF(at: arguments[1])
} else {
    print("Usage: swift pdf_ocr.swift <path_to_pdf>")
}
