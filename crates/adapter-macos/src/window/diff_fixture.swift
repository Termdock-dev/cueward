import Foundation
import CoreGraphics
import ImageIO

let directory = URL(fileURLWithPath: CommandLine.arguments[1])
for changed in [false, true] {
    var bytes = [UInt8](repeating: 255, count: 3 * 2 * 4)
    // CGImage provider rows are stored top first. Change column 1 in row 0.
    if changed { bytes[4] = 0; bytes[5] = 0; bytes[6] = 0 }
    let data = Data(bytes)
    guard let provider = CGDataProvider(data: data as CFData),
          let space = CGColorSpace(name: CGColorSpace.sRGB),
          let image = CGImage(width: 3, height: 2, bitsPerComponent: 8, bitsPerPixel: 32,
                              bytesPerRow: 12, space: space,
                              bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue),
                              provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent),
          let destination = CGImageDestinationCreateWithURL(directory.appendingPathComponent(changed ? "after.png" : "before.png") as CFURL, "public.png" as CFString, 1, nil) else { exit(1) }
    CGImageDestinationAddImage(destination, image, nil)
    if !CGImageDestinationFinalize(destination) { exit(1) }
}
