import CoreGraphics
import Foundation

private struct PatternTestFailure: Error {
    let message: String
}

private struct SyntheticPattern {
    let width: Int
    let height: Int
    let rowBytes: Int
    var bytes: [UInt8]

    func decode() -> LockPatternDecode {
        bytes.withUnsafeBytes { decodeLockPattern(width: width, height: height,
                                                  bytesPerRow: rowBytes, bgra: $0) }
    }

    mutating func paint(x: Int, y: Int, width: Int, height: Int, value: UInt8) {
        for row in y..<(y + height) {
            for column in x..<(x + width) {
                let offset = row * rowBytes + column * 4
                bytes[offset] = value
                bytes[offset + 1] = value
                bytes[offset + 2] = value
                bytes[offset + 3] = 255
            }
        }
    }
}

private func syntheticPattern(sequence: Int, scale: Int = 1,
                              top: Int = 0, left: Int = 0) throws -> SyntheticPattern {
    let width = 320 * scale + left * 2
    let height = 160 * scale + top
    let rowBytes = width * 4 + 16
    var bytes = [UInt8](repeating: 0, count: rowBytes * height)
    try bytes.withUnsafeMutableBytes { memory in
        let info = CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.premultipliedFirst.rawValue
        guard let context = CGContext(data: memory.baseAddress, width: width, height: height,
            bitsPerComponent: 8, bytesPerRow: rowBytes, space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: info) else { throw PatternTestFailure(message: "synthetic CGContext unavailable") }
        context.setFillColor(gray: 0.25, alpha: 1)
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        // Quartz's bottom-up coordinates map to top-down raw bitmap rows.
        context.translateBy(x: CGFloat(left), y: 0)
        context.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
        drawLockPattern(sequence: sequence, in: context)
    }
    return SyntheticPattern(width: width, height: height, rowBytes: rowBytes, bytes: bytes)
}

private func expect(_ image: SyntheticPattern, status: String, sequence: Int?) throws {
    let actual = image.decode()
    guard actual.status == status, actual.sequence == sequence else {
        throw PatternTestFailure(message: "expected \(status)/\(String(describing: sequence)), got " +
                                 "\(actual.status)/\(String(describing: actual.sequence))")
    }
}

@main private struct LockPatternTests {
    static func main() throws {
        for sequence in [1, 2, 257, 65535] {
            try expect(syntheticPattern(sequence: sequence), status: "decoded", sequence: sequence)
        }
        try expect(syntheticPattern(sequence: 531, scale: 2, top: 44, left: 12),
                   status: "decoded", sequence: 531)
        try expect(syntheticPattern(sequence: 8192, scale: 3, top: 69, left: 9),
                   status: "decoded", sequence: 8192)
        try rejectCorruption()
        try expect(syntheticPattern(sequence: 0), status: "blank", sequence: nil)
        let transparent = SyntheticPattern(width: 320, height: 160, rowBytes: 1280,
            bytes: [UInt8](repeating: 0, count: 1280 * 160))
        try expect(transparent, status: "blank", sequence: nil)
        let truncated = SyntheticPattern(width: 320, height: 160, rowBytes: 1280, bytes: [0])
        try expect(truncated, status: "undecodable", sequence: nil)
        let overflowingStride = SyntheticPattern(width: 320, height: 160,
            rowBytes: Int.max, bytes: [0])
        try expect(overflowingStride, status: "undecodable", sequence: nil)
        print("lock-pattern: 12 cases passed")
    }

    private static func rejectCorruption() throws {
        var checksum = try syntheticPattern(sequence: 257)
        // Flip one payload low-half cell, leaving its complement row untouched.
        checksum.paint(x: 16 + 16 * 8, y: 160 - 72, width: 8, height: 8, value: 0)
        try expect(checksum, status: "undecodable", sequence: nil)
        var magic = try syntheticPattern(sequence: 257)
        magic.paint(x: 16, y: 160 - 96, width: 8, height: 8, value: 0)
        try expect(magic, status: "undecodable", sequence: nil)
    }
}
