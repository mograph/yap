import Foundation
import UIKit

extension Profile {
    /// Your names and slang, used to bias recognition.
    var vocabulary: String {
        var seen = Set<String>()
        return (dictionary.map(\.write) + myWords)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty && seen.insert($0.lowercased()).inserted }
            .joined(separator: ", ")
    }
}

/// Everything about one dictation: mic -> text -> cleanup -> history, plus the keyboard session.
///
/// Keyboard session: iOS only lets an app turn the mic on while it's in front. So the first time
/// the keyboard needs it, it opens Yap, which starts the mic and keeps it running in the background
/// for a few minutes. After that the keyboard can start and stop dictation without leaving your app.
@MainActor
final class Engine: ObservableObject {
    enum Phase: Equatable {
        case idle
        case listening
        case thinking(String)
        case done
        case error(String)
    }

    @Published var phase: Phase = .idle
    @Published var level: Float = 0
    @Published private(set) var sessionLive = false
    @Published private(set) var fromKeyboard = false
    @Published var history: [Dictation]
    @Published var settings: Settings { didSet { Store.save(settings, as: "settings.json") } }
    @Published var profile: Profile { didSet { Store.save(profile, as: "profile.json") } }

    let transcriber = Transcriber()
    private let audio = AudioCapture()
    private var heartbeat: Timer?
    private var lastActivity = Date()
    /// How long the mic stays ready for the keyboard after the last dictation.
    let idleLimit: TimeInterval = 5 * 60

    init() {
        settings = Store.load("settings.json", as: Settings.self) ?? Settings()
        profile = Store.load("profile.json", as: Profile.self) ?? Profile()
        history = Store.load("history.json", as: [Dictation].self) ?? []
        audio.onLevel = { [weak self] rms in self?.level = rms }
        Bridge.observe("start") { [weak self] in Task { @MainActor in self?.keyboardStart() } }
        Bridge.observe("stop") { [weak self] in Task { @MainActor in self?.finish() } }
        Bridge.observe("cancel") { [weak self] in Task { @MainActor in self?.cancel() } }
        transcriber.refresh()
        publish("idle")
        Bridge.defaults.set(0, forKey: Bridge.Key.heartbeat)
    }

    // MARK: keyboard session

    /// moonshot://dictate from the keyboard's mic button: start the mic and listen right away.
    func openedFromKeyboard() {
        startSession()
        fromKeyboard = true
        startListening()
    }

    func startSession() {
        guard !sessionLive else { return }
        do {
            try audio.startEngine()
        } catch {
            fail("Couldn't start the microphone: \(error.localizedDescription)")
            return
        }
        sessionLive = true
        lastActivity = Date()
        heartbeat = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.beat() }
        }
        beat()
    }

    func endSession() {
        if phase == .listening { cancel() }
        heartbeat?.invalidate()
        heartbeat = nil
        sessionLive = false
        fromKeyboard = false
        audio.stopEngine()
        Bridge.defaults.set(0, forKey: Bridge.Key.heartbeat)
    }

    private func beat() {
        guard sessionLive else { return }
        let busy = phase == .listening || { if case .thinking = phase { return true } else { return false } }()
        if !busy && Date().timeIntervalSince(lastActivity) > idleLimit {
            endSession()
            return
        }
        Bridge.defaults.set(Date().timeIntervalSince1970, forKey: Bridge.Key.heartbeat)
    }

    private func keyboardStart() {
        guard sessionLive else { return }
        fromKeyboard = true
        startListening()
    }

    // MARK: dictation

    /// The big mic button in the app.
    func toggle() {
        switch phase {
        case .listening:
            finish()
        case .thinking:
            break
        default:
            fromKeyboard = false
            startSession()
            startListening()
        }
    }

    func startListening() {
        guard audio.isRunning, phase != .listening else { return }
        audio.beginCapture()
        phase = .listening
        lastActivity = Date()
        publish("listening")
        tap()
    }

    func cancel() {
        guard phase == .listening else { return }
        _ = audio.endCapture()
        phase = .idle
        publish("idle")
    }

    func finish() {
        guard phase == .listening else { return }
        let samples = audio.endCapture()
        let keyboard = fromKeyboard
        lastActivity = Date()
        level = 0
        tap()
        process(samples, keyboard: keyboard)
    }

    /// Transcribe, clean up, save, and hand the result to the keyboard (or the clipboard).
    func process(_ samples: [Float], keyboard: Bool) {
        let seconds = Double(samples.count) / 16_000
        let loud = samples.isEmpty ? 0 : (samples.reduce(0) { $0 + $1 * $1 } / Float(samples.count)).squareRoot()
        guard seconds > 0.3, loud > 0.002 else {
            fail("Didn't catch that")
            return
        }
        setThinking("Transcribing…")
        Task {
            do {
                let started = Date()
                let raw = try await transcriber.transcribe(
                    samples, id: settings.model, language: settings.language, vocabulary: profile.vocabulary)
                let sttMs = Int(Date().timeIntervalSince(started) * 1000)
                guard !raw.isEmpty else { throw Polish.Failure(message: "Didn't catch that") }

                setThinking("Making it sound like you…")
                var d = await Polish.run(settings: settings, profile: profile, raw: raw)
                d.audioSecs = seconds
                d.sttMs = sttMs
                history.insert(d, at: 0)
                if history.count > 2000 { history.removeLast(history.count - 2000) }
                saveHistory()

                Bridge.defaults.set(d.text, forKey: Bridge.Key.result)
                Bridge.defaults.set(d.id, forKey: Bridge.Key.resultId)
                if keyboard {
                    Bridge.defaults.set(true, forKey: Bridge.Key.pending)
                } else {
                    UIPasteboard.general.string = d.text
                }
                phase = .done
                publish("done")
            } catch {
                fail(error.localizedDescription)
            }
            lastActivity = Date()
        }
    }

    /// Dev hook for testing without a mic: launch with `-testWav <path> -testModel <id>` to
    /// transcribe a 16 kHz mono WAV, downloading the model first if needed.
    func runLaunchTest() async {
        guard let path = UserDefaults.standard.string(forKey: "testWav"),
              let data = FileManager.default.contents(atPath: path) else { return }
        let model = UserDefaults.standard.string(forKey: "testModel") ?? settings.model
        if !transcriber.isInstalled(model) { await transcriber.download(model) }
        settings.model = model
        process(Wav.samples(from: data), keyboard: false)
    }

    func saveHistory() { Store.save(history, as: "history.json") }

    func delete(_ d: Dictation) {
        history.removeAll { $0.id == d.id }
        saveHistory()
    }

    private func setThinking(_ message: String) {
        phase = .thinking(message)
        publish("thinking", message)
    }

    private func fail(_ message: String) {
        phase = .error(message)
        publish("error", message)
    }

    private func publish(_ state: String, _ message: String = "") {
        Bridge.defaults.set(state, forKey: Bridge.Key.state)
        Bridge.defaults.set(message, forKey: Bridge.Key.message)
    }

    private func tap() {
        if settings.haptics { UIImpactFeedbackGenerator(style: .medium).impactOccurred() }
    }
}
