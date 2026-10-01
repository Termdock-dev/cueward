// Independent integration check: the native PDF annotation must appear in PNG pixels.
import Foundation
import ImageIO
import CoreGraphics
let url = URL(fileURLWithPath: CommandLine.arguments[1])
let source = CGImageSourceCreateWithURL(url as CFURL, nil)!
let image = CGImageSourceCreateImageAtIndex(source, 0, nil)!
let context = CGContext(data: nil, width: image.width, height: image.height, bitsPerComponent: 8,
    bytesPerRow: image.width * 4, space: CGColorSpaceCreateDeviceRGB(),
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
let bytes = context.data!.assumingMemoryBound(to: UInt8.self)
var red = 0
for offset in stride(from: 0, to: image.width * image.height * 4, by: 4) {
    if bytes[offset] > 200 && bytes[offset + 1] < 100 && bytes[offset + 2] < 100 { red += 1 }
}
assert(red > 50, "PDF annotation missing from rendered pixels")
