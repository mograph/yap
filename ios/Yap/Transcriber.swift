import CoreML
import Foundation
import WhisperKit

/// Whisper on the iPhone via WhisperKit. It only uses the Neural Engine and CPU, never the GPU,
/// because iOS doesn't let apps touch the GPU from the background (keyboard dictation).
@MainActor
final class Transcriber: ObservableObject {
    struct Model: Identifiable {
        let id: String
        let label: String
        let sizeMB: Int
        let note: String
        let badge: String
        var english: Bool { id.contains(".en") }
    }

    static let models = [
        Model(id: "openai_whisper-large-v3-v20240930_turbo_632MB", label: "Whisper Turbo", sizeMB: 632,
              note: "Same as the Mac's default. Handles slang, names and fast talking, in 99 languages.", badge: "Best"),
        Model(id: "openai_whisper-small.en_217MB", label: "Whisper Small · English", sizeMB: 217,
              note: "Faster and smaller, a bit less accurate.", badge: ""),
        Model(id: "openai_whisper-base.en", label: "Whisper Base · English", sizeMB: 145,
              note: "Tiny and quick. Fine for short messages.", badge: ""),
    ]

    enum Status: Equatable {
        case notDownloaded
        case downloading(Double)
        case ready
        case failed(String)
    }

    @Published private(set) var status: [String: Status] = [:]

    private var kit: WhisperKit?
    private var loadedId: String?

    /// Models stay with the app (not the keyboard), so they live in the app's own storage.
    private static var downloadBase: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("whisper")
    }

    private func folder(_ id: String) -> String? {
        guard let path = UserDefaults.standard.string(forKey: "whisper.\(id)"),
              FileManager.default.fileExists(atPath: path) else { return nil }
        return path
    }

    func isInstalled(_ id: String) -> Bool { folder(id) != nil }

    func refresh() {
        for model in Self.models {
            if case .downloading = status[model.id] { continue }
            status[model.id] = isInstalled(model.id) ? .ready : .notDownloaded
        }
    }

    func download(_ id: String) async {
        status[id] = .downloading(0)
        do {
            try FileManager.default.createDirectory(at: Self.downloadBase, withIntermediateDirectories: true)
            let url = try await WhisperKit.download(variant: id, downloadBase: Self.downloadBase) { progress in
                let fraction = progress.fractionCompleted
                Task { @MainActor in self.status[id] = .downloading(fraction) }
            }
            UserDefaults.standard.set(url.path, forKey: "whisper.\(id)")
            status[id] = .ready
        } catch {
            status[id] = .failed(error.localizedDescription)
        }
    }

    func delete(_ id: String) {
        if let path = folder(id) { try? FileManager.default.removeItem(atPath: path) }
        UserDefaults.standard.removeObject(forKey: "whisper.\(id)")
        if loadedId == id {
            kit = nil
            loadedId = nil
        }
        status[id] = .notDownloaded
    }

    /// Loads the model once and keeps it, so each dictation starts instantly.
    func load(_ id: String) async throws -> WhisperKit {
        if let kit, loadedId == id { return kit }
        guard let path = folder(id) else {
            throw Polish.Failure(message: "Download a Whisper model in Settings first.")
        }
        kit = nil
        let compute = ModelComputeOptions(melCompute: .cpuOnly, audioEncoderCompute: .cpuAndNeuralEngine, textDecoderCompute: .cpuAndNeuralEngine)
        let config = WhisperKitConfig(model: id, modelFolder: path, computeOptions: compute, verbose: false, logLevel: .error, prewarm: true, load: true, download: false)
        let loaded = try await WhisperKit(config)
        kit = loaded
        loadedId = id
        return loaded
    }

    func transcribe(_ samples: [Float], id: String, language: String, vocabulary: String) async throws -> String {
        let whisper = try await load(id)
        let english = Self.models.first { $0.id == id }?.english ?? false
        let lang: String? = english ? "en" : (language == "auto" ? nil : language)
        var options = DecodingOptions(
            task: .transcribe, language: lang, temperatureFallbackCount: 3, usePrefillPrompt: true,
            detectLanguage: lang == nil, skipSpecialTokens: true, withoutTimestamps: true)
        // Bias recognition toward your names and slang, like the Mac app does.
        if !vocabulary.isEmpty, let tokenizer = whisper.tokenizer {
            options.promptTokens = tokenizer.encode(text: " Okay so, like, \(vocabulary).")
                .filter { $0 < tokenizer.specialTokens.specialTokenBegin }
        }
        // whisper wants at least a second of audio
        var audio = samples
        if audio.count < 16_000 { audio += [Float](repeating: 0, count: 16_000 - audio.count) }
        let results = try await whisper.transcribe(audioArray: audio, decodeOptions: options)
        return Self.tidy(results.map(\.text).joined(separator: " "))
    }

    private static let annotation = try! NSRegularExpression(
        pattern: "\\[[^\\]]*\\]|\\((?:silence|music|inaudible|laughs|laughing|applause|coughs?|sighs?|background noise|blank_audio)\\)",
        options: .caseInsensitive)

    /// Strips whisper's [BLANK_AUDIO]-style annotations and tidies spaces.
    static func tidy(_ text: String) -> String {
        let stripped = annotation.stringByReplacingMatches(in: text, range: LocalRules.full(text), withTemplate: " ")
        return stripped.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }
}
