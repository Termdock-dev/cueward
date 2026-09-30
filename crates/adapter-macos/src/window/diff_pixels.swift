import Foundation
import CoreGraphics
import ImageIO

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}
guard CommandLine.arguments.count == 5,
      let width = Int(CommandLine.arguments[3]),
      let height = Int(CommandLine.arguments[4]),
      width > 0, height > 0, width <= 16384, height <= 16384,
      width * height <= 16_777_216 else { fail("invalid or oversized comparison dimensions") }

func pixels(_ path: String) -> [UInt8] {
    let url = URL(fileURLWithPath: path)
    guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
          CGImageSourceGetType(source) as String? == "public.png",
          let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
          properties[kCGImagePropertyPixelWidth] as? Int == width,
          properties[kCGImagePropertyPixelHeight] as? Int == height,
          let image = CGImageSourceCreateImageAtIndex(source, 0, nil),
          image.width == width, image.height == height,
          let space = CGColorSpace(name: CGColorSpace.sRGB) else {
        fail("PNG missing, unreadable, or dimensions differ from snapshot")
    }
    var bytes = [UInt8](repeating: 0, count: width * height * 4)
    bytes.withUnsafeMutableBytes { buffer in
        let format = CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue
        guard let context = CGContext(data: buffer.baseAddress, width: width, height: height,
                                      bitsPerComponent: 8, bytesPerRow: width * 4,
                                      space: space, bitmapInfo: format) else { fail("cannot decode PNG pixels") }
        // Untransformed bitmap storage preserves the source's top-first rows.
        context.setBlendMode(.copy)
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
    }
    return bytes
}

let before = pixels(CommandLine.arguments[1])
let after = pixels(CommandLine.arguments[2])
var changed = 0
var minX = width, minY = height, maxX = -1, maxY = -1
for y in 0..<height {
    for x in 0..<width {
        let offset = (y * width + x) * 4
        if (0..<4).contains(where: { before[offset + $0] != after[offset + $0] }) {
            changed += 1
            minX = min(minX, x); minY = min(minY, y)
            maxX = max(maxX, x); maxY = max(maxY, y)
        }
    }
}
let bounds: Any = changed == 0 ? NSNull() : [
    "x": minX, "y": minY, "width": maxX - minX + 1, "height": maxY - minY + 1,
]
let result: [String: Any] = ["changed_pixels": changed, "total_pixels": width * height, "changed_bounds": bounds]
do {
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: result))
} catch { fail("cannot encode image comparison") }
