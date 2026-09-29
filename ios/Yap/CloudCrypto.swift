import CryptoKit
import Security
import Foundation

/// The half of cloud sync that has to agree with the Mac to the byte: which vault a passphrase
/// points at, and how a library is sealed. Mirrors the crypto in src-tauri/src/cloud.rs, and
/// `scripts/ios-parity.sh` seals on each side and opens on the other to prove it.
///
/// Your passphrase does two separate jobs. It names the vault (Argon2id with a fixed, public salt)
/// and it derives the key (Argon2id with a random salt stored next to the ciphertext). Different
/// derivations, so the vault id gives nothing away about the key.
enum CloudCrypto {
    /// Shortest passphrase allowed. With no account, the passphrase is the only thing keeping
    /// libraries apart, so it has to be long enough that a vault can't be found with a wordlist.
    static let minPassphrase = 12

    private static let obvious: Set<String> = [
        "password", "passphrase", "123456", "12345678", "123456789", "qwerty", "letmein", "welcome",
        "admin", "iloveyou", "monkey", "dragon", "sunshine", "princess", "football", "baseball",
        "abc123", "111111", "000000", "changeme", "secret", "yap", "yapyap", "dictation", "chloe",
    ]

    /// Refuses a passphrase that can't do the job. Same rules and same words as the Mac.
    static func check(_ passphrase: String) -> String? {
        let trimmed = passphrase.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.unicodeScalars.count < minPassphrase {
            return "That passphrase is too short. It needs at least \(minPassphrase) characters — it's what your library is encrypted with."
        }
        let folded = trimmed.lowercased()
        let withoutTrailingDigits = String(folded.unicodeScalars.reversed().drop { ("0"..."9").contains($0) }.reversed().map(Character.init))
        if obvious.contains(folded) || obvious.contains(withoutTrailingDigits) {
            return "That passphrase is one of the first things anyone would try. Pick something else."
        }
        // "aaaaaaaaaaaa" is twelve characters and no harder to guess than one.
        if Set(folded.unicodeScalars).count < 5 {
            return "That passphrase repeats too few characters to be hard to guess. Pick something else."
        }
        // A long PIN is still a PIN.
        if folded.unicodeScalars.allSatisfy({ ("0"..."9").contains($0) }) {
            return "That passphrase is only numbers, which is quick to guess. Add words or letters."
        }
        // "abcdefghijkl", "123456789012" and their reverses read as varied but are one guess each.
        let bytes = Array(folded.utf8)
        let runs = zip(bytes, bytes.dropFirst()).filter { abs(Int($1) - Int($0)) == 1 }.count
        if runs + 1 >= folded.unicodeScalars.count {
            return "That passphrase is a straight run of characters. Pick something less predictable."
        }
        return nil
    }

    /// Names the vault on the passphrase-only path. Never used to encrypt.
    static func vaultId(_ passphrase: String) -> String {
        Argon2id.hash(password: Array(passphrase.utf8), salt: Array("yap-vault-id-v1".utf8), length: 16)
            .map { String(format: "%02x", $0) }
            .joined()
    }

    static func key(_ passphrase: String, salt: [UInt8]) -> SymmetricKey {
        SymmetricKey(data: Argon2id.hash(password: Array(passphrase.utf8), salt: salt, length: 32))
    }

    /// Encrypts a library: AES-256-GCM, ciphertext with its tag appended, which is what the Rust
    /// `aes-gcm` crate writes. A fresh salt and nonce every time.
    static func seal(_ passphrase: String, _ lib: Library) throws -> (salt: String, nonce: String, blob: String) {
        let salt = try random(16)
        return try seal(key(passphrase, salt: salt), salt: salt, lib)
    }

    /// Sealing with a key that's already derived, for when the same salt is reused. The nonce is
    /// always fresh, so a key never meets the same nonce twice.
    static func seal(_ key: SymmetricKey, salt: [UInt8], _ lib: Library) throws -> (salt: String, nonce: String, blob: String) {
        let plain = try JSONEncoder().encode(lib)
        let nonce = try random(12)
        let box = try AES.GCM.seal(plain, using: key, nonce: AES.GCM.Nonce(data: nonce))
        return (base64url(Data(salt)), base64url(Data(nonce)), base64url(box.ciphertext + box.tag))
    }

    /// Decrypts a library. A wrong passphrase fails here rather than returning nonsense.
    static func unseal(_ passphrase: String, salt: String, nonce: String, blob: String) throws -> Library {
        guard let salt = unbase64url(salt) else { throw Polish.Failure(message: "The library in the cloud is damaged.") }
        return try unseal(key(passphrase, salt: Array(salt)), nonce: nonce, blob: blob)
    }

    static func unseal(_ key: SymmetricKey, nonce: String, blob: String) throws -> Library {
        guard let nonce = unbase64url(nonce), let blob = unbase64url(blob), nonce.count == 12, blob.count >= 16 else {
            throw Polish.Failure(message: "The library in the cloud is damaged.")
        }
        let box = try AES.GCM.SealedBox(nonce: AES.GCM.Nonce(data: nonce), ciphertext: blob.dropLast(16), tag: blob.suffix(16))
        guard let plain = try? AES.GCM.open(box, using: key) else {
            throw Polish.Failure(message: "That passphrase doesn't open the library in the cloud.")
        }
        guard let lib = try? JSONDecoder().decode(Library.self, from: plain) else {
            throw Polish.Failure(message: "The library in the cloud isn't readable.")
        }
        return lib
    }

    private static func random(_ count: Int) throws -> [UInt8] {
        var bytes = [UInt8](repeating: 0, count: count)
        guard SecRandomCopyBytes(kSecRandomDefault, count, &bytes) == errSecSuccess else {
            throw Polish.Failure(message: "Couldn't encrypt your library.")
        }
        return bytes
    }

    /// URL-safe base64 without padding, as the Mac writes it.
    static func base64url(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    static func unbase64url(_ s: String) -> Data? {
        var b = s.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        b += String(repeating: "=", count: (4 - b.count % 4) % 4)
        return Data(base64Encoded: b)
    }
}
