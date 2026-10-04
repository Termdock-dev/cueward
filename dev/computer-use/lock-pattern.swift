import CoreGraphics
import Foundation

/// A visual counter, not a provenance/authentication mechanism.
struct LockPatternDecode: Sendable {
    let status: String
    let sequence: Int?
}

private let lockPatternMagic: UInt32 = 0xD59A6C3B

/// Draw in a 320x160 unflipped content view. The CGContext uses bottom-up points.
func drawLockPattern(sequence: Int, in context: CGContext) {
    context.saveGState()
    defer { context.restoreGState() }
    context.setShouldAntialias(false)
    context.setFillColor(gray: 0.5, alpha: 1)
    context.fill(CGRect(x: 0, y: 0, width: 320, height: 160))
    guard (1...65535).contains(sequence) else { return }
    let counter = UInt32(sequence)
    let payload = (counter << 16) | (counter ^ 0xFFFF)
    drawLockPatternRow(lockPatternMagic, y: 88, in: context)
    drawLockPatternRow(~lockPatternMagic, y: 48, in: context)
    drawLockPatternRow(payload, y: 64, in: context)
    drawLockPatternRow(~payload, y: 72, in: context)
}

private func drawLockPatternRow(_ value: UInt32, y: Int, in context: CGContext) {
    for index in 0..<32 {
        let bit = (value >> (31 - index)) & 1
        context.setFillColor(gray: CGFloat(bit), alpha: 1)
        context.fill(CGRect(x: 16 + index * 8, y: y, width: 8, height: 8))
    }
}

/// Read only temporary BGRA bytes; no image/pixel data is retained in the result.
func decodeLockPattern(width: Int, height: Int, bytesPerRow: Int,
                       bgra: UnsafeRawBufferPointer) -> LockPatternDecode {
    guard width > 0, height > 0, width <= 4096, height <= 4096,
          bytesPerRow >= width * 4, bytesPerRow <= bgra.count / height else {
        return LockPatternDecode(status: "undecodable", sequence: nil)
    }
    let pixels = LockPatternPixels(width: width, height: height,
                                   bytesPerRow: bytesPerRow, bytes: bgra)
    if pixels.isBlank() { return LockPatternDecode(status: "blank", sequence: nil) }
    for scale in 1...4 {
        if let sequence = findLockPattern(pixels, scale: scale) {
            return LockPatternDecode(status: "decoded", sequence: sequence)
        }
    }
    return LockPatternDecode(status: "undecodable", sequence: nil)
}

private struct LockPatternPixels {
    let width: Int
    let height: Int
    let bytesPerRow: Int
    let bytes: UnsafeRawBufferPointer

    func bit(x: Int, y: Int) -> UInt32? {
        guard x >= 0, x < width, y >= 0, y < height else { return nil }
        let offset = y * bytesPerRow + x * 4
        guard bytes[offset + 3] >= 208 else { return nil }
        let blue = Int(bytes[offset])
        let green = Int(bytes[offset + 1])
        let red = Int(bytes[offset + 2])
        let low = min(blue, green, red)
        let high = max(blue, green, red)
        guard high - low <= 24 else { return nil }
        if high <= 48 { return 0 }
        if low >= 208 { return 1 }
        return nil
    }

    func row(x: Int, y: Int, cell: Int) -> UInt32? {
        var value: UInt32 = 0
        for index in 0..<32 {
            guard let bit = bit(x: x + index * cell, y: y) else { return nil }
            value = (value << 1) | bit
        }
        return value
    }

    func isBlank() -> Bool {
        var low = 255
        var high = 0
        var opaque = false
        for y in stride(from: 0, to: height, by: max(1, height / 32)) {
            for x in stride(from: 0, to: width, by: max(1, width / 32)) {
                let offset = y * bytesPerRow + x * 4
                if bytes[offset + 3] < 16 { continue }
                opaque = true
                for channel in 0..<3 {
                    low = min(low, Int(bytes[offset + channel]))
                    high = max(high, Int(bytes[offset + channel]))
                }
            }
        }
        return !opaque || high - low <= 8
    }
}

private func findLockPattern(_ pixels: LockPatternPixels, scale: Int) -> Int? {
    let cell = 8 * scale
    let maximumX = pixels.width - 31 * cell - 1
    let maximumY = pixels.height - 40 * scale - 1
    guard maximumX >= 0, maximumY >= 0 else { return nil }
    for y in 0...maximumY {
        for x in 0...maximumX {
            guard pixels.row(x: x, y: y, cell: cell) == lockPatternMagic,
                  pixels.row(x: x, y: y + 40 * scale, cell: cell) == ~lockPatternMagic,
                  let payload = pixels.row(x: x, y: y + 24 * scale, cell: cell),
                  pixels.row(x: x, y: y + 16 * scale, cell: cell) == ~payload else { continue }
            let sequence = payload >> 16
            guard sequence > 0, payload & 0xFFFF == sequence ^ 0xFFFF else { continue }
            return Int(sequence)
        }
    }
    return nil
}
