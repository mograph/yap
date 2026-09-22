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
/// the keyboard needs it, it opens Moonshot, which starts the mic and keeps it running in the
/// background for a few minutes. After that the keyboard can start and stop dictation without
/// leaving your app.
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
    @Published var profile: Profile {
        didSet {
            // Only a real edit counts as newer, like the Mac; otherwise synced devices would
            // ping-pong a profile that never changed.
            if !applyingSync && !profile.sameContent(as: oldValue) && profile.updatedAt == oldValue.updatedAt {
                profile.updatedAt = Store.nowMs()
            }
            Store.save(profile, as: "profile.json")
        }
    }
    /// Dictations deleted here or anywhere else, so syncing never brings them back.
    private(set) var deleted: [String]
    private var applyingSync = false

    // Cloud sync. The account holds the passphrase, so views only ever see these flags.
    @Published private var account: Cloud.Account
    @Published private(set) var cloudBusy = false
    @Published var cloudStatus = ""
    @Published var cloudError = ""
    @Published var showFirebaseDetails = false
    @Published private(set) var lastSynced: Date?
    private var syncing = false
    var cloudRegistered: Bool { account.registered }
    var cloudAnonymous: Bool { account.anonymous }
    var cloudEmail: String { account.email }
    var cloudHasPassphrase: Bool { !account.passphrase.isEmpty }

    let transcriber = Transcriber()
    private let audio = AudioCapture()
    private var heartbeat: Timer?
    private var syncTimer: Timer?
    private var lastActivity = Date()
    /// How long the mic stays ready for the keyboard after the last dictation.
    let idleLimit: TimeInterval = 5 * 60

    init() {
        settings = Store.load("settings.json", as: Settings.self) ?? Settings()
        profile = Store.load("profile.json", as: Profile.self) ?? Profile()
        history = Store.load("history.json", as: [Dictation].self) ?? []
        deleted = Store.load("deleted.json", as: [String].self) ?? []
        account = Cloud.loadAccount()
        audio.onLevel = { [weak self] rms in self?.level = rms }
        Bridge.observe("start") { [weak self] in Task { @MainActor in self?.keyboardStart() } }
        Bridge.observe("stop") { [weak self] in Task { @MainActor in self?.finish() } }
        Bridge.observe("cancel") { [weak self] in Task { @MainActor in self?.cancel() } }
        Bridge.observe("pickList") { [weak self] in Task { @MainActor in self?.keyboardPickedList() } }
        transcriber.refresh()
        publish("idle")
        Bridge.defaults.set(0, forKey: Bridge.Key.heartbeat)
        // Picks up what your other devices did, the way the Mac checks in every couple of minutes.
        syncTimer = Timer.scheduledTimer(withTimeInterval: 120, repeats: true) { [weak self] _ in
            Task { @MainActor in await self?.syncCloud(explicit: false) }
        }
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
                if history.count > LibrarySync.limit { history.removeLast(history.count - LibrarySync.limit) }
                saveHistory()

                Bridge.defaults.set(d.text, forKey: Bridge.Key.result)
                Bridge.defaults.set(d.list, forKey: Bridge.Key.resultList)
                Bridge.defaults.set(d.id, forKey: Bridge.Key.resultId)
                // In "ask" mode the keyboard offers the list before it types, like the Mac's pill.
                Bridge.defaults.set(settings.lists == "ask" && !d.list.isEmpty, forKey: Bridge.Key.offer)
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
            await syncCloud(explicit: false)
        }
    }

    /// The keyboard offered the list and you took it: record it the way the Mac does.
    private func keyboardPickedList() {
        guard let id = Bridge.defaults.string(forKey: Bridge.Key.resultId),
              let i = history.firstIndex(where: { $0.id == id }), !history[i].list.isEmpty
        else { return }
        var d = history[i]
        d.edits.append(Edit(original: "(as said)", replacement: d.list.contains("- ") ? "bullet list" : "grouped by subject",
                            kind: "formatting", applied: true, why: "you picked it"))
        d.text = d.list
        d.list = ""
        d.words = LocalRules.words(d.text).count
        history[i] = d
        saveHistory()
    }

    /// "Make it a list" on a dictation in the app: swap in the tidier version and copy it.
    func takeList(_ item: Dictation) {
        guard let i = history.firstIndex(where: { $0.id == item.id }), !history[i].list.isEmpty else { return }
        history[i].text = history[i].list
        history[i].list = ""
        history[i].words = LocalRules.words(history[i].text).count
        UIPasteboard.general.string = history[i].text
        saveHistory()
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
    private func saveDeleted() { Store.save(deleted, as: "deleted.json") }

    func delete(_ d: Dictation) {
        history.removeAll { $0.id == d.id }
        deleted.append(d.id)
        saveDeleted()
        saveHistory()
    }

    /// Clearing is deleting everything, so it's remembered and a sync won't bring it all back.
    func clearHistory() {
        deleted += history.map(\.id)
        history.removeAll()
        saveDeleted()
        saveHistory()
    }

    // MARK: library

    var library: Library { Library(profile: profile, history: history, deleted: deleted) }

    /// Adds another library to yours, from here or from the Mac. Never removes anything.
    func importLibrary(_ data: Data) -> String {
        do {
            let lib = try LibrarySync.read(data)
            var merged = profile
            let words = LibrarySync.addProfile(&merged, lib.profile)
            if words > 0 { profile = merged }
            let dictations = LibrarySync.mergeHistory(&history, lib.history, deleted: deleted)
            saveHistory()
            Task { await syncCloud(explicit: false) }
            return "Added \(words) word\(words == 1 ? "" : "s") and \(dictations) dictation\(dictations == 1 ? "" : "s")"
        } catch {
            return error.localizedDescription
        }
    }

    // MARK: cloud

    private func setAccount(_ a: Cloud.Account) {
        account = a
        Cloud.saveAccount(a)
    }

    private func cloudCall(_ run: () async throws -> String) async {
        cloudBusy = true
        cloudStatus = ""
        cloudError = ""
        do {
            cloudStatus = try await run()
        } catch {
            cloudError = error.localizedDescription
        }
        cloudBusy = false
    }

    func usePassphraseOnly() async {
        await cloudCall {
            let fresh = try await Cloud.registerAnonymously(apiKey: settings.firebaseApiKey)
            // Keep the passphrase already set here: it's what decrypts what's already uploaded.
            setAccount(Cloud.Account(uid: fresh.uid, refreshToken: fresh.refreshToken, anonymous: true, passphrase: account.passphrase))
            return "Set up with a passphrase. No account needed."
        }
    }

    func signInWithGoogle() async {
        // A greyed-out button just looks broken. Say what's missing and open the place to fix it.
        guard !settings.googleClientId.trimmingCharacters(in: .whitespaces).isEmpty else {
            showFirebaseDetails = true
            cloudStatus = ""
            cloudError = "Google sign-in needs an iOS OAuth client. Add its client ID under Firebase details, or use a passphrase instead."
            return
        }
        await cloudCall {
            let fresh = try await Cloud.signInWithGoogle(clientId: settings.googleClientId, apiKey: settings.firebaseApiKey)
            setAccount(Cloud.Account(uid: fresh.uid, email: fresh.email, refreshToken: fresh.refreshToken,
                                     anonymous: false, passphrase: account.passphrase))
            return "Signed in as \(fresh.email)"
        }
    }

    /// The passphrase encrypts the library either way, and on the passphrase-only path it also
    /// picks the vault, so it's checked before it's kept. Returns whether it was saved.
    @discardableResult
    func setPassphrase(_ passphrase: String) -> Bool {
        cloudStatus = ""
        cloudError = ""
        if let problem = CloudCrypto.check(passphrase) {
            cloudError = problem
            return false
        }
        var a = account
        a.passphrase = passphrase.trimmingCharacters(in: .whitespacesAndNewlines)
        setAccount(a)
        cloudStatus = "Passphrase saved. Use the same one everywhere."
        return true
    }

    /// Forgets this phone's registration and passphrase. Nothing here is deleted, and what's in the
    /// cloud stays for whatever still has the passphrase or the account.
    func forgetCloud() {
        setAccount(Cloud.Account())
        cloudError = ""
        cloudStatus = "Signed out on this phone. Nothing here was touched."
    }

    func syncNow() async { await syncCloud(explicit: true) }

    /// Syncs with Firestore. Automatic syncs respect the toggle and stay quiet; "Sync now" always
    /// runs and says what happened.
    func syncCloud(explicit: Bool) async {
        let a = account
        guard !syncing, a.registered, !a.passphrase.isEmpty, explicit || settings.cloudSync else { return }
        syncing = true
        if explicit {
            cloudBusy = true
            cloudStatus = ""
            cloudError = ""
        }
        defer {
            syncing = false
            cloudBusy = false
        }
        do {
            let result = try await Cloud.sync(projectId: settings.firebaseProjectId, apiKey: settings.firebaseApiKey,
                                              account: a, local: library)
            // Fold the result into what's here *now*, rather than replacing it: a dictation made
            // while the sync was on the network would otherwise vanish.
            if let remote = result.library {
                var p = profile, h = history, del = deleted
                _ = LibrarySync.merge(remote, profile: &p, history: &h, deleted: &del)
                applyingSync = true
                profile = p
                applyingSync = false
                history = h
                deleted = del
                saveHistory()
                saveDeleted()
            }
            lastSynced = Date()
            if explicit {
                cloudStatus = result.changed ? "Synced. This phone picked up changes from your other devices."
                    : result.uploaded ? "Synced. Your library is now in the cloud." : "Already up to date."
            }
        } catch {
            if explicit { cloudError = error.localizedDescription }
        }
    }

    // MARK: status

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
