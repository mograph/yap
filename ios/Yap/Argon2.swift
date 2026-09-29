import Foundation

/// Argon2id, which CryptoKit doesn't have. Cloud sync derives the vault id and the encryption key
/// from your passphrase with it, and the phone has to land on exactly the bytes the Mac does, so
/// this uses the same parameters as the Rust `argon2` crate's defaults (19 MiB, 2 passes, 1 lane,
/// version 0x13) and `scripts/ios-parity.sh` checks the output against it.
///
/// Follows RFC 9106 and the reference implementation, specialised to one lane, which is all these
/// parameters use. Memory-hard on purpose: run it off the main thread.
enum Argon2id {
    static let memoryKiB: UInt32 = 19 * 1024
    static let passes: UInt32 = 2
    static let lanes: UInt32 = 1
    static let version: UInt32 = 0x13
    static let typeId: UInt32 = 2

    private static let blockWords = 128
    private static let syncPoints = 4

    static func hash(password: [UInt8], salt: [UInt8], length: Int) -> [UInt8] {
        // H0: every parameter, then the password and salt, each with its length.
        var h = Blake2b(outputLength: 64)
        for value in [lanes, UInt32(length), memoryKiB, passes, version, typeId] { h.update(le32(value)) }
        h.update(le32(UInt32(password.count)))
        h.update(password)
        h.update(le32(UInt32(salt.count)))
        h.update(salt)
        h.update(le32(0)) // no secret
        h.update(le32(0)) // no associated data
        let h0 = h.finalize()

        let blockCount = Int(memoryKiB) // already a multiple of 4 lanes' worth
        let laneLength = blockCount
        let segmentLength = laneLength / syncPoints
        var memory = [UInt64](repeating: 0, count: blockCount * blockWords)

        // The first two blocks of the lane come straight from H0.
        for j in 0..<2 {
            let bytes = blake2bLong(h0 + le32(UInt32(j)) + le32(0), length: 1024)
            for w in 0..<blockWords { memory[j * blockWords + w] = loadLE64(bytes, w * 8) }
        }

        memory.withUnsafeMutableBufferPointer { buffer in
            let m = buffer.baseAddress!
            let scratch = UnsafeMutablePointer<UInt64>.allocate(capacity: blockWords * 5)
            defer { scratch.deallocate() }
            let r = scratch, tmp = scratch + blockWords
            let address = scratch + 2 * blockWords, input = scratch + 3 * blockWords, zero = scratch + 4 * blockWords

            for pass in 0..<Int(passes) {
                for slice in 0..<syncPoints {
                    // Argon2id: the first half of the first pass picks blocks independently of the
                    // data (resisting side channels), everything after depends on it (resisting GPUs).
                    let independent = pass == 0 && slice < syncPoints / 2
                    if independent {
                        for w in 0..<blockWords { input[w] = 0; zero[w] = 0 }
                        input[0] = UInt64(pass)
                        input[1] = 0 // lane
                        input[2] = UInt64(slice)
                        input[3] = UInt64(blockCount)
                        input[4] = UInt64(passes)
                        input[5] = UInt64(typeId)
                    }
                    var start = 0
                    if pass == 0 && slice == 0 {
                        start = 2
                        if independent { nextAddresses(address, input, zero, r, tmp) }
                    }
                    var current = slice * segmentLength + start
                    var previous = current % laneLength == 0 ? current + laneLength - 1 : current - 1

                    for i in start..<segmentLength {
                        if current % laneLength == 1 { previous = current - 1 }
                        let pseudoRandom: UInt64
                        if independent {
                            if i % blockWords == 0 { nextAddresses(address, input, zero, r, tmp) }
                            pseudoRandom = address[i % blockWords]
                        } else {
                            pseudoRandom = m[previous * blockWords]
                        }

                        // Which earlier block to mix in. One lane, so always this one.
                        let area: UInt64
                        if pass == 0 {
                            area = UInt64(slice == 0 ? i - 1 : slice * segmentLength + i - 1)
                        } else {
                            area = UInt64(laneLength - segmentLength + i - 1)
                        }
                        var relative = pseudoRandom & 0xFFFF_FFFF
                        relative = (relative &* relative) >> 32
                        relative = area &- 1 &- ((area &* relative) >> 32)
                        let base = pass == 0 ? 0 : (slice == syncPoints - 1 ? 0 : (slice + 1) * segmentLength)
                        let reference = (base + Int(relative)) % laneLength

                        fill(prev: m + previous * blockWords, ref: m + reference * blockWords,
                             next: m + current * blockWords, xor: pass != 0, r: r, tmp: tmp)
                        current += 1
                        previous += 1
                    }
                }
            }
        }

        var last = [UInt8](repeating: 0, count: 1024)
        for w in 0..<blockWords { storeLE64(&last, w * 8, memory[(laneLength - 1) * blockWords + w]) }
        return blake2bLong(last, length: length)
    }

    // MARK: the compression function G

    private static func nextAddresses(_ address: UnsafeMutablePointer<UInt64>, _ input: UnsafeMutablePointer<UInt64>,
                                      _ zero: UnsafeMutablePointer<UInt64>, _ r: UnsafeMutablePointer<UInt64>,
                                      _ tmp: UnsafeMutablePointer<UInt64>) {
        input[6] &+= 1
        fill(prev: zero, ref: input, next: address, xor: false, r: r, tmp: tmp)
        fill(prev: zero, ref: address, next: address, xor: false, r: r, tmp: tmp)
    }

    private static func fill(prev: UnsafeMutablePointer<UInt64>, ref: UnsafeMutablePointer<UInt64>,
                             next: UnsafeMutablePointer<UInt64>, xor: Bool,
                             r: UnsafeMutablePointer<UInt64>, tmp: UnsafeMutablePointer<UInt64>) {
        for k in 0..<blockWords {
            r[k] = ref[k] ^ prev[k]
            tmp[k] = r[k]
        }
        // From the second pass on, version 0x13 mixes in what the block held before.
        if xor { for k in 0..<blockWords { tmp[k] ^= next[k] } }
        for i in 0..<8 {
            let o = 16 * i
            round(r, o, o + 1, o + 2, o + 3, o + 4, o + 5, o + 6, o + 7,
                  o + 8, o + 9, o + 10, o + 11, o + 12, o + 13, o + 14, o + 15)
        }
        for i in 0..<8 {
            let o = 2 * i
            round(r, o, o + 1, o + 16, o + 17, o + 32, o + 33, o + 48, o + 49,
                  o + 64, o + 65, o + 80, o + 81, o + 96, o + 97, o + 112, o + 113)
        }
        for k in 0..<blockWords { next[k] = tmp[k] ^ r[k] }
    }

    @inline(__always)
    private static func round(_ v: UnsafeMutablePointer<UInt64>, _ a0: Int, _ a1: Int, _ a2: Int, _ a3: Int,
                              _ a4: Int, _ a5: Int, _ a6: Int, _ a7: Int, _ a8: Int, _ a9: Int, _ a10: Int,
                              _ a11: Int, _ a12: Int, _ a13: Int, _ a14: Int, _ a15: Int) {
        gb(v, a0, a4, a8, a12)
        gb(v, a1, a5, a9, a13)
        gb(v, a2, a6, a10, a14)
        gb(v, a3, a7, a11, a15)
        gb(v, a0, a5, a10, a15)
        gb(v, a1, a6, a11, a12)
        gb(v, a2, a7, a8, a13)
        gb(v, a3, a4, a9, a14)
    }

    /// BLAKE2b's mixing step, with Argon2's multiplication folded in so it can't be skipped cheaply.
    @inline(__always)
    private static func gb(_ v: UnsafeMutablePointer<UInt64>, _ a: Int, _ b: Int, _ c: Int, _ d: Int) {
        v[a] = blamka(v[a], v[b]); v[d] = rotr(v[d] ^ v[a], 32)
        v[c] = blamka(v[c], v[d]); v[b] = rotr(v[b] ^ v[c], 24)
        v[a] = blamka(v[a], v[b]); v[d] = rotr(v[d] ^ v[a], 16)
        v[c] = blamka(v[c], v[d]); v[b] = rotr(v[b] ^ v[c], 63)
    }

    @inline(__always)
    private static func blamka(_ x: UInt64, _ y: UInt64) -> UInt64 {
        x &+ y &+ 2 &* ((x & 0xFFFF_FFFF) &* (y & 0xFFFF_FFFF))
    }

    @inline(__always)
    static func rotr(_ x: UInt64, _ n: UInt64) -> UInt64 { (x >> n) | (x << (64 - n)) }

    // MARK: H', BLAKE2b stretched to any length

    static func blake2bLong(_ input: [UInt8], length: Int) -> [UInt8] {
        let prefix = le32(UInt32(length))
        if length <= 64 {
            var h = Blake2b(outputLength: length)
            h.update(prefix)
            h.update(input)
            return h.finalize()
        }
        var h = Blake2b(outputLength: 64)
        h.update(prefix)
        h.update(input)
        var v = h.finalize()
        var out = Array(v[0..<32])
        var remaining = length - 32
        while remaining > 64 {
            var next = Blake2b(outputLength: 64)
            next.update(v)
            v = next.finalize()
            out += v[0..<32]
            remaining -= 32
        }
        var tail = Blake2b(outputLength: remaining)
        tail.update(v)
        out += tail.finalize()
        return out
    }

    static func le32(_ v: UInt32) -> [UInt8] { [UInt8(v & 0xff), UInt8((v >> 8) & 0xff), UInt8((v >> 16) & 0xff), UInt8(v >> 24)] }

    static func loadLE64(_ b: [UInt8], _ at: Int) -> UInt64 {
        var v: UInt64 = 0
        for i in 0..<8 { v |= UInt64(b[at + i]) << (8 * UInt64(i)) }
        return v
    }

    static func storeLE64(_ b: inout [UInt8], _ at: Int, _ v: UInt64) {
        for i in 0..<8 { b[at + i] = UInt8((v >> (8 * UInt64(i))) & 0xff) }
    }
}

/// BLAKE2b (RFC 7693), unkeyed, any output length up to 64 bytes.
struct Blake2b {
    private static let iv: [UInt64] = [
        0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1,
        0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179,
    ]
    private static let sigma: [[Int]] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
        [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
        [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
        [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
        [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
        [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
        [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
        [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
        [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    ]

    private var h: [UInt64]
    private var counter: (UInt64, UInt64) = (0, 0)
    private var buffer = [UInt8]()
    private let outputLength: Int

    init(outputLength: Int) {
        self.outputLength = outputLength
        h = Self.iv
        h[0] ^= 0x0101_0000 ^ UInt64(outputLength)
        buffer.reserveCapacity(128)
    }

    mutating func update(_ data: [UInt8]) {
        for byte in data {
            // The last block is compressed differently, so a full buffer waits for more input.
            if buffer.count == 128 {
                advance(128)
                compress(buffer, last: false)
                buffer.removeAll(keepingCapacity: true)
            }
            buffer.append(byte)
        }
    }

    mutating func finalize() -> [UInt8] {
        advance(UInt64(buffer.count))
        let block = buffer + [UInt8](repeating: 0, count: 128 - buffer.count)
        compress(block, last: true)
        var out = [UInt8](repeating: 0, count: 64)
        for (i, word) in h.enumerated() { Argon2id.storeLE64(&out, i * 8, word) }
        return Array(out[0..<outputLength])
    }

    private mutating func advance(_ n: UInt64) {
        let (low, overflow) = counter.0.addingReportingOverflow(n)
        counter = (low, counter.1 &+ (overflow ? 1 : 0))
    }

    private mutating func compress(_ block: [UInt8], last: Bool) {
        var m = [UInt64](repeating: 0, count: 16)
        for i in 0..<16 { m[i] = Argon2id.loadLE64(block, i * 8) }
        var v = h + Self.iv
        v[12] ^= counter.0
        v[13] ^= counter.1
        if last { v[14] = ~v[14] }
        func g(_ a: Int, _ b: Int, _ c: Int, _ d: Int, _ x: UInt64, _ y: UInt64) {
            v[a] = v[a] &+ v[b] &+ x; v[d] = Argon2id.rotr(v[d] ^ v[a], 32)
            v[c] = v[c] &+ v[d]; v[b] = Argon2id.rotr(v[b] ^ v[c], 24)
            v[a] = v[a] &+ v[b] &+ y; v[d] = Argon2id.rotr(v[d] ^ v[a], 16)
            v[c] = v[c] &+ v[d]; v[b] = Argon2id.rotr(v[b] ^ v[c], 63)
        }
        for s in Self.sigma {
            g(0, 4, 8, 12, m[s[0]], m[s[1]])
            g(1, 5, 9, 13, m[s[2]], m[s[3]])
            g(2, 6, 10, 14, m[s[4]], m[s[5]])
            g(3, 7, 11, 15, m[s[6]], m[s[7]])
            g(0, 5, 10, 15, m[s[8]], m[s[9]])
            g(1, 6, 11, 12, m[s[10]], m[s[11]])
            g(2, 7, 8, 13, m[s[12]], m[s[13]])
            g(3, 4, 9, 14, m[s[14]], m[s[15]])
        }
        for i in 0..<8 { h[i] ^= v[i] ^ v[i + 8] }
    }
}
