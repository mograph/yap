import Foundation

/// Settings, voice profile and history, shared by the app and the keyboard through the App Group.
/// The JSON matches the Mac app's files so they can sync later.
enum Store {
    static let group = "group.io.tinkerstudio.yap"

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

struct Settings: Codable {
    /// WhisperKit model folder name. Transcription always happens on the iPhone.
    var model = "openai_whisper-large-v3-v20240930_turbo_632MB"
    /// "auto" or an ISO code like "en"
    var language = "auto"
    /// "claude" or "local"
    var brain = "claude"
    var claudeModel = "claude-opus-5"
    var anthropicKey = ""
    var haptics = true

    enum CodingKeys: String, CodingKey { case model, language, brain, claudeModel, anthropicKey, haptics }
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
    }
}

struct DictEntry: Codable, Hashable {
    var say = ""
    var write = ""
}

struct Profile: Codable {
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

    enum CodingKeys: String, CodingKey { case name, tone, casual, rules, myWords, dictionary, samples, notes }

    func rule(_ kind: String) -> Rule {
        kind == "dictionary" ? .change : (rules[kind] ?? .change)
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

    enum CodingKeys: String, CodingKey {
        case id, createdAt, raw, text, edits, engine, note, words, keptPct, audioSecs, sttMs, polishMs
    }

    var date: Date { ISO8601DateFormatter().date(from: createdAt) ?? Date() }
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
    }
}
