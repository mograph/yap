import AuthenticationServices
import CryptoKit
import Foundation
import Security
import UIKit

/// Cloud sync on the phone: the same Firestore vaults, the same encryption and the same merge as
/// the Mac (src-tauri/src/cloud.rs), so your library flows between them either way.
///
/// Two ways to set up, same as the Mac: just a passphrase (registered anonymously, the passphrase
/// picks the vault) or signing in with Google (your account picks it, Firebase keeps it yours).
enum Cloud {
    /// Shown everywhere Google sign-in appears. Same words as the Mac.
    static let googleWarning = "Google sign-in uses a Firebase project under Chloe Ward's account (yap-tinkerstudio) that hasn't been migrated yet. Signing in adds your Google account to it."

    /// This phone's registration and the passphrase. Kept in the Keychain, on this device only:
    /// never in the App Group, never in iCloud, never uploaded.
    struct Account: Codable, Equatable {
        var uid = ""
        /// Empty unless signed in with Google.
        var email = ""
        var refreshToken = ""
        /// True when nobody signed in: the passphrase decides which vault is yours.
        var anonymous = false
        var passphrase = ""

        var registered: Bool { !refreshToken.isEmpty }
    }

    struct Synced {
        var changed = false
        var uploaded = false
        var library: Library?
    }

    // MARK: account

    private static let service = "io.tinkerstudio.moonshot.cloud"

    static func loadAccount() -> Account {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: "account",
            kSecReturnData as String: true,
        ]
        var found: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &found) == errSecSuccess, let data = found as? Data else { return Account() }
        return (try? JSONDecoder().decode(Account.self, from: data)) ?? Account()
    }

    static func saveAccount(_ account: Account) {
        let base: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: "account",
        ]
        SecItemDelete(base as CFDictionary)
        guard account != Account(), let data = try? JSONEncoder().encode(account) else { return }
        var add = base
        add[kSecValueData as String] = data
        // Readable after the first unlock so a sync can finish in the background; never leaves the phone.
        add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        SecItemAdd(add as CFDictionary, nil)
    }

    // MARK: talking to Firebase

    private static func enc(_ s: String) -> String {
        s.addingPercentEncoding(withAllowedCharacters: CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~")) ?? s
    }

    private static func form(_ pairs: [(String, String)]) -> Data {
        Data(pairs.map { "\($0.0)=\(enc($0.1))" }.joined(separator: "&").utf8)
    }

    private static func post(_ url: String, _ body: Data, form: Bool) async throws -> [String: Any] {
        var request = URLRequest(url: URL(string: url)!)
        request.httpMethod = "POST"
        request.httpBody = body
        request.setValue(form ? "application/x-www-form-urlencoded" : "application/json", forHTTPHeaderField: "Content-Type")
        let (data, response): (Data, URLResponse)
        do {
            (data, response) = try await URLSession.shared.data(for: request)
        } catch {
            throw Polish.Failure(message: "Couldn't reach Google: \(error.localizedDescription)")
        }
        let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        guard ((response as? HTTPURLResponse)?.statusCode ?? 0) < 300 else {
            let message = ((json["error"] as? [String: Any])?["message"] as? String)
                ?? json["error_description"] as? String ?? "unknown error"
            // The setup steps people miss, named plainly rather than left as API codes.
            if message.contains("ADMIN_ONLY_OPERATION") {
                throw Polish.Failure(message: "Turn on Anonymous sign-in in the Firebase console, then try again.")
            }
            if message.contains("OPERATION_NOT_ALLOWED") {
                throw Polish.Failure(message: "Turn on the Google sign-in provider in the Firebase console, then try again.")
            }
            throw Polish.Failure(message: "Google refused: \(message)")
        }
        return json
    }

    /// Registers this phone without anyone signing in. Firebase hands back an anonymous user so its
    /// rules have something to check; it isn't an account and it doesn't decide which vault you see.
    static func registerAnonymously(apiKey: String) async throws -> Account {
        let json = try await post("https://identitytoolkit.googleapis.com/v1/accounts:signUp?key=\(enc(apiKey))",
                                  Data(#"{"returnSecureToken":true}"#.utf8), form: false)
        return Account(uid: json["localId"] as? String ?? "", refreshToken: json["refreshToken"] as? String ?? "", anonymous: true)
    }

    /// Signs in with Google in a system sheet, so your password goes to Google and never to Moonshot,
    /// then trades Google's token for a Firebase session.
    @MainActor
    static func signInWithGoogle(clientId: String, apiKey: String) async throws -> Account {
        let id = clientId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard id.hasSuffix(".apps.googleusercontent.com") else {
            throw Polish.Failure(message: "That doesn't look like a Google iOS client ID. It ends in .apps.googleusercontent.com.")
        }
        // Google's iOS clients redirect to the client ID reversed, as a URL scheme.
        let scheme = id.split(separator: ".").reversed().joined(separator: ".")
        let redirect = scheme + ":/oauth2redirect"
        let verifier = CloudCrypto.base64url(Data((0..<48).map { _ in UInt8.random(in: 0...255) }))
        let challenge = CloudCrypto.base64url(Data(SHA256.hash(data: Data(verifier.utf8))))
        let state = CloudCrypto.base64url(Data((0..<16).map { _ in UInt8.random(in: 0...255) }))

        var auth = URLComponents(string: "https://accounts.google.com/o/oauth2/v2/auth")!
        auth.queryItems = [
            .init(name: "client_id", value: id), .init(name: "redirect_uri", value: redirect),
            .init(name: "response_type", value: "code"), .init(name: "scope", value: "openid email profile"),
            .init(name: "code_challenge", value: challenge), .init(name: "code_challenge_method", value: "S256"),
            .init(name: "state", value: state), .init(name: "prompt", value: "select_account"),
        ]
        let callback = try await GoogleSheet().run(auth.url!, scheme: scheme)
        let items = URLComponents(url: callback, resolvingAgainstBaseURL: false)?.queryItems ?? []
        if let error = items.first(where: { $0.name == "error" })?.value { throw Polish.Failure(message: "Google said: \(error)") }
        // A mismatched state means this redirect wasn't the one we started.
        guard items.first(where: { $0.name == "state" })?.value == state,
              let code = items.first(where: { $0.name == "code" })?.value
        else { throw Polish.Failure(message: "Sign-in didn't match the request that started it.") }

        let token = try await post("https://oauth2.googleapis.com/token", form([
            ("code", code), ("client_id", id), ("code_verifier", verifier),
            ("grant_type", "authorization_code"), ("redirect_uri", redirect),
        ]), form: true)
        guard let idToken = token["id_token"] as? String else { throw Polish.Failure(message: "Google didn't return an ID token.") }

        let body = try JSONSerialization.data(withJSONObject: [
            "postBody": "id_token=\(idToken)&providerId=google.com",
            "requestUri": "http://localhost",
            "returnSecureToken": true,
        ])
        let session = try await post("https://identitytoolkit.googleapis.com/v1/accounts:signInWithIdp?key=\(enc(apiKey))", body, form: false)
        return Account(uid: session["localId"] as? String ?? "", email: session["email"] as? String ?? "",
                       refreshToken: session["refreshToken"] as? String ?? "", anonymous: false)
    }

    /// A short-lived token for Firestore, from the long-lived refresh token.
    private static func idToken(apiKey: String, refreshToken: String) async throws -> String {
        let json = try await post("https://securetoken.googleapis.com/v1/token?key=\(enc(apiKey))",
                                  form([("grant_type", "refresh_token"), ("refresh_token", refreshToken)]), form: true)
        guard let token = json["id_token"] as? String, !token.isEmpty else {
            throw Polish.Failure(message: "This phone's Firebase session expired. Sign in again, or forget this phone and set it up again.")
        }
        return token
    }

    // MARK: Firestore

    private static func docURL(_ project: String, _ path: String) -> URL {
        URL(string: "https://firestore.googleapis.com/v1/projects/\(project)/databases/(default)/documents/\(path)")!
    }

    private static func firestore(_ method: String, _ url: URL, token: String, body: Data? = nil) async throws -> (Int, [String: Any]) {
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.httpBody = body
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
            return ((response as? HTTPURLResponse)?.statusCode ?? 0, json)
        } catch {
            throw Polish.Failure(message: "Couldn't reach Firestore: \(error.localizedDescription)")
        }
    }

    private static func refused(_ json: [String: Any]) -> Polish.Failure {
        Polish.Failure(message: "Firestore refused: \((json["error"] as? [String: Any])?["message"] as? String ?? "unknown error")")
    }

    /// Two-way sync with Firestore: pull, decrypt, merge, push back if this phone knows something
    /// the cloud doesn't. Same merge rules as the Mac, so they converge whoever syncs first.
    static func sync(projectId: String, apiKey: String, account: Account, local: Library) async throws -> Synced {
        guard !projectId.trimmingCharacters(in: .whitespaces).isEmpty, !apiKey.trimmingCharacters(in: .whitespaces).isEmpty else {
            throw Polish.Failure(message: "Add your Firebase project and API key in Settings first.")
        }
        guard !account.passphrase.isEmpty else {
            throw Polish.Failure(message: "Set a passphrase in Settings. Your library is encrypted with it before it leaves this phone.")
        }
        if let problem = CloudCrypto.check(account.passphrase) { throw Polish.Failure(message: problem) }

        let keys = Keys.shared
        let path = account.anonymous ? "vaults/\(await keys.vault(account.passphrase))" : "users/\(account.uid)/library/main"
        let url = docURL(projectId, path)
        let token = try await idToken(apiKey: apiKey, refreshToken: account.refreshToken)

        let (status, doc) = try await firestore("GET", url, token: token)
        var remote: Library?
        if status == 404 {
            remote = nil // nothing synced from anywhere yet
        } else if status >= 300 {
            throw refused(doc)
        } else {
            let fields = doc["fields"] as? [String: Any] ?? [:]
            let field = { (name: String) in (fields[name] as? [String: Any])?["stringValue"] as? String ?? "" }
            let (salt, nonce, blob) = (field("salt"), field("nonce"), field("blob"))
            if !salt.isEmpty && !nonce.isEmpty && !blob.isEmpty {
                remote = try await keys.open(account.passphrase, salt: salt, nonce: nonce, blob: blob)
            }
        }

        var profile = local.profile, history = local.history, deleted = local.deleted
        let (changed, merged) = LibrarySync.merge(remote, profile: &profile, history: &history, deleted: &deleted)
        let uploaded = LibrarySync.differs(remote, merged)
        if uploaded {
            let sealed = try await keys.seal(account.passphrase, merged)
            let body = try JSONSerialization.data(withJSONObject: ["fields": [
                "app": ["stringValue": "yap"],
                "v": ["integerValue": "1"],
                "salt": ["stringValue": sealed.salt],
                "nonce": ["stringValue": sealed.nonce],
                "blob": ["stringValue": sealed.blob],
                "updatedAt": ["integerValue": String(Store.nowMs())],
            ]])
            let (put, reply) = try await firestore("PATCH", url, token: token, body: body)
            guard put < 300 else { throw refused(reply) }
        }
        return Synced(changed: changed, uploaded: uploaded, library: changed ? merged : nil)
    }

    /// Argon2id costs a moment per key, so each passphrase and salt is derived once per launch.
    /// Sealing reuses one salt for the session; each upload still gets a fresh nonce, so no two
    /// uploads are the same bytes and the key is never used with a repeated nonce.
    actor Keys {
        static let shared = Keys()
        private var vaults: [String: String] = [:]
        private var keys: [String: SymmetricKey] = [:]
        private var sealSalt: [String: [UInt8]] = [:]

        func vault(_ passphrase: String) -> String {
            if let v = vaults[passphrase] { return v }
            let v = CloudCrypto.vaultId(passphrase)
            vaults[passphrase] = v
            return v
        }

        private func key(_ passphrase: String, _ salt: [UInt8]) -> SymmetricKey {
            let id = passphrase + "\u{0}" + salt.map { String(format: "%02x", $0) }.joined()
            if let k = keys[id] { return k }
            let k = CloudCrypto.key(passphrase, salt: salt)
            keys[id] = k
            return k
        }

        func open(_ passphrase: String, salt: String, nonce: String, blob: String) throws -> Library {
            guard let s = CloudCrypto.unbase64url(salt) else { throw Polish.Failure(message: "The library in the cloud is damaged.") }
            return try CloudCrypto.unseal(key(passphrase, Array(s)), nonce: nonce, blob: blob)
        }

        func seal(_ passphrase: String, _ lib: Library) throws -> (salt: String, nonce: String, blob: String) {
            let salt = sealSalt[passphrase] ?? (0..<16).map { _ in UInt8.random(in: 0...255) }
            sealSalt[passphrase] = salt
            return try CloudCrypto.seal(key(passphrase, salt), salt: salt, lib)
        }
    }
}

/// Presents Google's sign-in page in the system's authentication sheet.
@MainActor
private final class GoogleSheet: NSObject, ASWebAuthenticationPresentationContextProviding {
    private var session: ASWebAuthenticationSession?

    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows).first { $0.isKeyWindow } ?? ASPresentationAnchor()
    }

    func run(_ url: URL, scheme: String) async throws -> URL {
        try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(url: url, callbackURLScheme: scheme) { callback, error in
                if let callback {
                    continuation.resume(returning: callback)
                } else if (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin {
                    continuation.resume(throwing: Polish.Failure(message: "Sign-in was cancelled."))
                } else {
                    continuation.resume(throwing: Polish.Failure(message: "Sign-in failed: \(error?.localizedDescription ?? "unknown error")"))
                }
            }
            session.presentationContextProvider = self
            self.session = session
            session.start()
        }
    }
}
