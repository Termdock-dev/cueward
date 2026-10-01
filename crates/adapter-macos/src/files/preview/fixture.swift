import Foundation
import CoreGraphics
import CoreText
import PDFKit
import ImageIO

let directory = URL(fileURLWithPath: CommandLine.arguments[1])
func text(_ value: String, context: CGContext, x: CGFloat, y: CGFloat) {
    let font = CTFontCreateWithName("Helvetica" as CFString, 44, nil)
    let attrs: [NSAttributedString.Key: Any] = [NSAttributedString.Key(kCTFontAttributeName as String): font,
        NSAttributedString.Key(kCTForegroundColorAttributeName as String): CGColor(gray: 0, alpha: 1)]
    let line = CTLineCreateWithAttributedString(NSAttributedString(string: value, attributes: attrs))
    context.textPosition = CGPoint(x: x, y: y)
    CTLineDraw(line, context)
}
func image() -> CGImage {
    let context = CGContext(data: nil, width: 800, height: 300, bitsPerComponent: 8,
        bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.setFillColor(CGColor(gray: 1, alpha: 1))
    context.fill(CGRect(x: 0, y: 0, width: 800, height: 300))
    text("SCAN EXAMPLE 123", context: context, x: 30, y: 160)
    return context.makeImage()!
}
let scan = image()
let png = CGImageDestinationCreateWithURL(directory.appendingPathComponent("scan.png") as CFURL, "public.png" as CFString, 1, nil)!
CGImageDestinationAddImage(png, scan, nil)
assert(CGImageDestinationFinalize(png))
let pdfUrl = directory.appendingPathComponent("mixed.pdf")
var bounds = CGRect(x: 0, y: 0, width: 800, height: 300)
let pdf = CGContext(pdfUrl as CFURL, mediaBox: &bounds, nil)!
pdf.beginPDFPage(nil)
text("NATIVE EXAMPLE 456", context: pdf, x: 30, y: 160)
pdf.endPDFPage()
pdf.beginPDFPage(nil)
pdf.draw(scan, in: bounds)
pdf.endPDFPage()
pdf.beginPDFPage(nil)
pdf.endPDFPage()
pdf.closePDF()
let document = PDFDocument(url: pdfUrl)!
assert(document.write(to: directory.appendingPathComponent("locked.pdf"), withOptions: [.ownerPasswordOption: "owner", .userPasswordOption: "secret"]))
let empty = PDFDocument()
if let data = empty.dataRepresentation() { try data.write(to: directory.appendingPathComponent("empty.pdf")) }
let rotated = PDFDocument(url: pdfUrl)!
rotated.page(at: 0)!.rotation = 90
assert(rotated.write(to: directory.appendingPathComponent("rotated.pdf")))
try Data("%PDF-1.7\ninvalid".utf8).write(to: directory.appendingPathComponent("damaged.pdf"))
try Data("not a document".utf8).write(to: directory.appendingPathComponent("unknown.bin"))
let nativeOnly = PDFDocument(url: pdfUrl)!
nativeOnly.removePage(at: 2)
nativeOnly.removePage(at: 1)
assert(nativeOnly.write(to: directory.appendingPathComponent("native-only.pdf")))

let annotated = PDFDocument(url: pdfUrl)!
let annotation = PDFAnnotation(bounds: CGRect(x: 710, y: 220, width: 70, height: 70), forType: .square, withProperties: nil)
annotation.color = .red
annotation.interiorColor = .red
annotated.page(at: 0)!.addAnnotation(annotation)
assert(annotated.write(to: directory.appendingPathComponent("annotated.pdf")))
