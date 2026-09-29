import Foundation

// The Swift half of the crypto parity check. Same modes and shapes as the Rust `parity_crypto`.
let args = CommandLine.arguments
let mode = args[1]
let input = try! JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: args[2]))) as! [String: Any]
let out: Any

func hex(_ b: [UInt8]) -> String { b.map { String(format: "%02x", $0) }.joined() }

switch mode {
case "derive":
    let keys = (input["keys"] as! [[String: String]]).map { k -> [String: String] in
        let pass = Array(k["pass"]!.utf8), salt = Array(k["salt"]!.utf8)
        return ["pass": k["pass"]!, "salt": k["salt"]!,
                "k16": hex(Argon2id.hash(password: pass, salt: salt, length: 16)),
                "k32": hex(Argon2id.hash(password: pass, salt: salt, length: 32)),
                "vault": CloudCrypto.vaultId(k["pass"]!)]
    }
    let checks = (input["passphrases"] as! [String]).map { ["pass": $0, "verdict": CloudCrypto.check($0) ?? ""] }
    out = ["keys": keys, "checks": checks]
case "seal":
    let lib = try! JSONDecoder().decode(Library.self, from: JSONSerialization.data(withJSONObject: input["library"]!))
    let sealed = try! CloudCrypto.seal(input["passphrase"] as! String, lib)
    out = ["salt": sealed.salt, "nonce": sealed.nonce, "blob": sealed.blob]
default:
    let s = input["sealed"] as! [String: String]
    let lib = try! CloudCrypto.unseal(input["passphrase"] as! String, salt: s["salt"]!, nonce: s["nonce"]!, blob: s["blob"]!)
    out = try! JSONSerialization.jsonObject(with: JSONEncoder().encode(lib))
}
try! JSONSerialization.data(withJSONObject: out, options: [.prettyPrinted, .sortedKeys]).write(to: URL(fileURLWithPath: args[3]))
