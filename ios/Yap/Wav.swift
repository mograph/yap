import Foundation

enum Wav {
    /// The samples in a 16-bit mono WAV (for the `-testWav` launch test).
    static func samples(from data: Data) -> [Float] {
        guard let marker = data.range(of: Data("data".utf8)) else { return [] }
        let start = marker.upperBound + 4
        guard start < data.count else { return [] }
        return data.withUnsafeBytes { bytes in
            stride(from: start, to: data.count - 1, by: 2).map {
                Float(Int16(littleEndian: bytes.loadUnaligned(fromByteOffset: $0, as: Int16.self))) / 32768
            }
        }
    }
}
