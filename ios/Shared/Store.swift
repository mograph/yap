import Foundation

/// Settings, voice profile and history, shared by the app and the keyboard through the App Group.
/// The JSON matches the Mac app's files, so a library moves between the two either way.
enum Store {
    static let group = "group.io.tinkerstudio.moonshot"

    static var folder: URL {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)
            ?? FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    }

    static func load<T: Decodable>(_ name: String, as type: T.Type) -> T? {
        guard let data = try? Data(contentsOf: folder.appendingPathComponent(name)) else { return nil }
        return try? JSONDecoder().decode(T.self, from: data)
    }

    static func save<T: Encodable>(_ value: T, as name: String) {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(value) else { return }
        try? data.write(to: folder.appendingPathComponent(name), options: .atomic)
    }

    static func nowMs() -> Int64 { Int64((Date().timeIntervalSince1970 * 1000).rounded()) }
}

extension KeyedDecodingContainer {
    /// Missing or unreadable keys fall back to the default instead of failing the whole file.
    func get<T: Decodable>(_ key: Key, _ fallback: T) -> T {
        (try? decodeIfPresent(T.self, forKey: key)) ?? fallback
    }
}

/// What the user allows for one kind of change.
enum Rule: String, Codable, CaseIterable {
    case change = "do"
    case suggest
    case leave
}

struct Settings: Codable, Equatable {
    /// WhisperKit model folder name. Transcription always happens on the iPhone.
    var model = "openai_whisper-large-v3-v20240930_turbo_632MB"
    /// "auto" or an ISO code like "en"
    var language = "auto"
    /// "claude" or "local"
    var brain = "claude"
    var claudeModel = "claude-opus-5"
    var anthropicKey = ""
    var haptics = true
    /// When there's a tidier arrangement on offer: "ask", "auto" (always take it) or "never"
    var lists = "ask"
    /// Keep an encrypted copy of the library in Firestore, shared with the Mac.
    var cloudSync = false
    var firebaseProjectId = "yap-tinkerstudio"
    /// Not a secret: Firebase web API keys identify the project, security rules do the guarding.
    var firebaseApiKey = "AIzaSyALock_PLlhBo6sZgQPCPBsoKMQI7b4tko"
    /// From an OAuth "iOS" client. Only needed to sign in with Google.
    var googleClientId = ""

    enum CodingKeys: String, CodingKey {
        case model, language, brain, claudeModel, anthropicKey, haptics, lists
        case cloudSync, firebaseProjectId, firebaseApiKey, googleClientId
    }
}

extension Settings {
    init(from decoder: Decoder) throws {
        self.init()
        let c = try decoder.container(keyedBy: CodingKeys.self)
        model = c.get(.model, model)
        language = c.get(.language, language)
        brain = c.get(.brain, brain)
        claudeModel = c.get(.claudeModel, claudeModel)
        anthropicKey = c.get(.anthropicKey, anthropicKey)
        haptics = c.get(.haptics, haptics)
        lists = c.get(.lists, lists)
        cloudSync = c.get(.cloudSync, cloudSync)
        // Blank was never anyone's choice, so it falls back to the project too, as on the Mac.
        firebaseProjectId = c.get(.firebaseProjectId, "").isEmpty ? firebaseProjectId : c.get(.firebaseProjectId, "")
        firebaseApiKey = c.get(.firebaseApiKey, "").isEmpty ? firebaseApiKey : c.get(.firebaseApiKey, "")
        googleClientId = c.get(.googleClientId, googleClientId)
    }
}

struct DictEntry: Codable, Hashable {
    var say = ""
    var write = ""
}

struct Profile: Codable, Equatable {
    var name = ""
    /// 0 = exactly how I talk, 100 = polished
    var tone = 25
    /// Write it like a Slack message: no structure imposed, never more formal than chatty.
    var casual = false
    var rules: [String: Rule] = [
        "filler": .change, "correction": .change, "punctuation": .change, "grammar": .suggest,
        "slang": .leave, "rephrase": .suggest, "swearing": .leave, "formatting": .change,
    ]
    var myWords = ["gonna", "wanna", "kinda"]
    var dictionary = [DictEntry(say: "tinker studio", write: "Tinker Studio"), DictEntry(say: "claude code", write: "Claude Code")]
    var samples = ""
    var notes = ""
    /// Milliseconds since 1970 of the last real edit; the newest wins when libraries sync.
    var updatedAt: Int64 = 0

    enum CodingKeys: String, CodingKey { case name, tone, casual, rules, myWords, dictionary, samples, notes, updatedAt }

    func rule(_ kind: String) -> Rule {
        kind == "dictionary" ? .change : (rules[kind] ?? .change)
    }

    /// Everything but the timestamp, so "did anything actually change?" isn't fooled by it.
    func sameContent(as other: Profile) -> Bool {
        var a = self, b = other
        a.updatedAt = 0
        b.updatedAt = 0
        return a == b
    }
}

extension Profile {
    init(from decoder: Decoder) throws {
        self.init()
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = c.get(.name, name)
        tone = c.get(.tone, tone)
        casual = c.get(.casual, casual)
        rules = c.get(.rules, rules)
        myWords = c.get(.myWords, myWords)
        dictionary = c.get(.dictionary, dictionary)
        samples = c.get(.samples, samples)
        notes = c.get(.notes, notes)
        updatedAt = c.get(.updatedAt, updatedAt)
    }
}

struct Edit: Codable, Hashable {
    var original: String
    var replacement: String
    var kind: String
    /// true = made the change, false = only suggested
    var applied: Bool
    var why: String
}

struct Dictation: Codable, Identifiable, Hashable {
    var id = UUID().uuidString
    var createdAt = ISO8601DateFormatter().string(from: Date())
    var raw = ""
    var text = ""
    var edits: [Edit] = []
    var engine = ""
    var note = ""
    var words = 0
    var keptPct: Double = 100
    var audioSecs: Double = 0
    var sttMs = 0
    var polishMs = 0
    /// The tidier arrangement, when there was one and it wasn't used
    var list = ""

    enum CodingKeys: String, CodingKey {
        case id, createdAt, raw, text, edits, engine, note, words, keptPct, audioSecs, sttMs, polishMs, list
    }

    var date: Date { Self.parse(createdAt) ?? Date() }

    private static let plain = ISO8601DateFormatter()
    private static let fractional: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    /// Reads both clocks: "2026-09-19T10:00:00Z" from here, and the Mac's
    /// "2026-09-19T10:00:00.123456789+00:00", whose nanoseconds ISO8601DateFormatter refuses.
    static func parse(_ stamp: String) -> Date? {
        if let d = plain.date(from: stamp) ?? fractional.date(from: stamp) { return d }
        guard let dot = stamp.firstIndex(of: "."),
              let zone = stamp[dot...].firstIndex(where: { $0 == "Z" || $0 == "+" || $0 == "-" })
        else { return nil }
        let millis = stamp[stamp.index(after: dot)..<zone].prefix(3)
        return fractional.date(from: String(stamp[..<dot]) + "." + millis + stamp[zone...])
    }
}

extension Dictation {
    init(from decoder: Decoder) throws {
        self.init()
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = c.get(.id, id)
        createdAt = c.get(.createdAt, createdAt)
        raw = c.get(.raw, raw)
        text = c.get(.text, text)
        edits = c.get(.edits, edits)
        engine = c.get(.engine, engine)
        note = c.get(.note, note)
        words = c.get(.words, words)
        keptPct = c.get(.keptPct, keptPct)
        audioSecs = c.get(.audioSecs, audioSecs)
        sttMs = c.get(.sttMs, sttMs)
        polishMs = c.get(.polishMs, polishMs)
        list = c.get(.list, list)
    }

    /// Like the Mac, an unused list is left out of the file rather than written as "".
    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(id, forKey: .id)
        try c.encode(createdAt, forKey: .createdAt)
        try c.encode(raw, forKey: .raw)
        try c.encode(text, forKey: .text)
        try c.encode(edits, forKey: .edits)
        try c.encode(engine, forKey: .engine)
        try c.encode(note, forKey: .note)
        try c.encode(words, forKey: .words)
        try c.encode(keptPct, forKey: .keptPct)
        try c.encode(audioSecs, forKey: .audioSecs)
        try c.encode(sttMs, forKey: .sttMs)
        try c.encode(polishMs, forKey: .polishMs)
        if !list.isEmpty { try c.encode(list, forKey: .list) }
    }
}

/// Voice profile, history and deletions in one file: what export, import and cloud sync move
/// around. Same shape as the Mac's `yap-library.json`, so the two read each other's.
struct Library: Codable {
    var app = "yap"
    var version = 1
    var profile = Profile()
    var history: [Dictation] = []
    /// Dictations deleted anywhere, so syncing doesn't bring them back.
    var deleted: [String] = []

    enum CodingKeys: String, CodingKey { case app, version, profile, history, deleted }
}

extension Library {
    init(from decoder: Decoder) throws {
        self.init()
        let c = try decoder.container(keyedBy: CodingKeys.self)
        app = c.get(.app, "")
        version = c.get(.version, version)
        profile = c.get(.profile, profile)
        history = c.get(.history, history)
        deleted = c.get(.deleted, deleted)
    }
}
