import SwiftUI
import UniformTypeIdentifiers

struct SettingsView: View {
    @EnvironmentObject var engine: Engine
    @State private var confirmClear = false
    @State private var passphrase = ""
    @State private var exporting = false
    @State private var importing = false
    @State private var libraryStatus = ""

    private static let claudeModels = [
        ("claude-opus-5", "Claude Opus 5", "Best at sounding like you"),
        ("claude-sonnet-5", "Claude Sonnet 5", "Faster, cheaper"),
        ("claude-haiku-4-5", "Claude Haiku 4.5", "Fastest"),
    ]
    private static let languages = [
        ("auto", "Detect automatically"), ("en", "English"), ("es", "Spanish"), ("fr", "French"), ("de", "German"),
        ("it", "Italian"), ("pt", "Portuguese"), ("zh", "Chinese"), ("ja", "Japanese"), ("ko", "Korean"), ("hi", "Hindi"),
    ]

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    NavigationLink {
                        KeyboardSetupView()
                    } label: {
                        Label("Set up the Moonshot keyboard", systemImage: "keyboard")
                    }
                    if engine.sessionLive {
                        Button("Turn off the mic now", systemImage: "mic.slash", role: .destructive) { engine.endSession() }
                    }
                } footer: {
                    Text("After you use the keyboard, the mic stays ready for 5 minutes so you don't have to jump to Moonshot again.")
                }

                listsSection
                accountSection
                librarySection

                Section {
                    ForEach(Transcriber.models) { model in modelRow(model) }
                    if !(Transcriber.models.first { $0.id == engine.settings.model }?.english ?? false) {
                        Picker("Language", selection: $engine.settings.language) {
                            ForEach(Self.languages, id: \.0) { Text($0.1).tag($0.0) }
                        }
                    }
                } header: {
                    Text("Ears")
                } footer: {
                    Text("Whisper runs on your iPhone. Private and free. Your voice never leaves your phone.")
                }

                Section {
                    Picker("Cleanup", selection: $engine.settings.brain) {
                        Text("Claude").tag("claude")
                        Text("Local rules only").tag("local")
                    }
                    .pickerStyle(.segmented)
                    if engine.settings.brain == "claude" {
                        SecureField("Anthropic API key (sk-ant-…)", text: $engine.settings.anthropicKey)
                        Picker("Model", selection: $engine.settings.claudeModel) {
                            ForEach(Self.claudeModels, id: \.0) { Text($0.1).tag($0.0) }
                        }
                        Link("Get a key", destination: URL(string: "https://console.anthropic.com/settings/keys")!)
                    }
                } header: {
                    Text("Brain")
                } footer: {
                    Text(engine.settings.brain == "claude"
                         ? "Claude rewrites your words in your voice: drops padding, fixes “no wait”s, makes bullet lists. Sends the text only, never audio."
                         : "Removes um/uh and stutters, follows “bullet point” and “new line”, spots lists and brain dumps, capitalizes. Works offline.")
                }

                Section {
                    Toggle("Haptics", isOn: $engine.settings.haptics)
                    Button(confirmClear ? "Tap again to clear \(engine.history.count) dictations" : "Clear history", role: .destructive) {
                        if confirmClear {
                            engine.clearHistory()
                            confirmClear = false
                        } else {
                            confirmClear = true
                        }
                    }
                } footer: {
                    Text("Cleared dictations stay cleared on your other devices too.")
                }
            }
            .navigationTitle("Settings")
            .onAppear { engine.transcriber.refresh() }
        }
    }

    // MARK: lists and formatting

    private var tier: Tone.Tier { Tone.tier(engine.profile.tone, casual: engine.profile.casual) }

    private var listsSection: some View {
        Section {
            HStack {
                Text("How polished?").font(.body.weight(.semibold))
                Spacer()
                Pill(text: tier.name, fg: Theme.accent, bg: Theme.accentSoft)
            }
            Slider(value: Binding(get: { Double(engine.profile.tone) }, set: { engine.profile.tone = Int($0) }), in: 0...100)
            if engine.profile.casual && engine.profile.tone > Tone.casualCap {
                Label("Casual mode caps this at relaxed.", systemImage: "exclamationmark.triangle")
                    .font(.caption).foregroundStyle(Theme.warn)
            }

            Toggle(isOn: $engine.profile.casual) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Casual mode").font(.body.weight(.semibold))
                    Text("Treat every dictation as a message, not a document. Nothing gets turned into bullets or grouped by subject.")
                        .font(.caption).foregroundStyle(Theme.ink2)
                }
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("When you run through several things").font(.body.weight(.semibold))
                Text(engine.profile.casual
                     ? "Off while casual mode is on."
                     : "Say “I'm going to make a list”, “let me do a brain dump”, or just run through a few things, and Moonshot can offer a tidier version. What you said is never changed unless you take it.")
                    .font(.caption).foregroundStyle(Theme.ink2)
                Picker("Lists", selection: $engine.settings.lists) {
                    Text("Offer it").tag("ask")
                    Text("Just do it").tag("auto")
                    Text("Never").tag("never")
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }
            .opacity(engine.profile.casual ? 0.5 : 1)
            .disabled(engine.profile.casual)
            .padding(.vertical, 4)

            VStack(alignment: .leading, spacing: 8) {
                Text("Bullets and paragraphs").font(.body.weight(.semibold))
                Text("Spoken commands like “bullet point”, “new line” and “new paragraph”.")
                    .font(.caption).foregroundStyle(Theme.ink2)
                Picker("Formatting", selection: Binding(get: { engine.profile.rule("formatting") },
                                                        set: { engine.profile.rules["formatting"] = $0 })) {
                    Text("Change it").tag(Rule.change)
                    Text("Suggest").tag(Rule.suggest)
                    Text("Leave it").tag(Rule.leave)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }
            .padding(.vertical, 4)

            VStack(alignment: .leading, spacing: 8) {
                Text("YOU SAY").font(.caption2.weight(.semibold)).foregroundStyle(Theme.ink3)
                Text("Let me do a brain dump. The login page is broken. The settings need work. I owe Priya an email.")
                    .font(.subheadline).foregroundStyle(Theme.ink2)
                Text("MOONSHOT OFFERS").font(.caption2.weight(.semibold)).foregroundStyle(Theme.ink3).padding(.top, 4)
                FormattedText(text: "Let me do a brain dump:\n- The login page is broken\n- The settings need work\n- I owe Priya an email")
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 12).fill(Theme.card2))
        } header: {
            Text("Lists and formatting")
        } footer: {
            Text("The same settings as Your voice, shown together because they work as a set.")
        }
    }

    // MARK: account

    private var accountSection: some View {
        Section {
            if engine.cloudRegistered {
                HStack {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(engine.cloudAnonymous ? "Set up with a passphrase" : "Signed in with Google").font(.body.weight(.semibold))
                        Text(engine.cloudAnonymous ? "No account. Any device with the same passphrase finds this library." : engine.cloudEmail)
                            .font(.caption).foregroundStyle(Theme.ink2)
                    }
                    Spacer()
                    Button("Sign out", role: .destructive) { engine.forgetCloud() }
                        .buttonStyle(.borderless)
                }
                if !engine.cloudAnonymous { googleWarning }
            } else {
                Button {
                    Task { await engine.signInWithGoogle() }
                } label: {
                    Label("Sign in with Google", systemImage: "person.crop.circle.badge.checkmark")
                }
                .disabled(engine.cloudBusy)
                googleWarning
                Button {
                    Task { await engine.usePassphraseOnly() }
                } label: {
                    Label("Use a passphrase, no account", systemImage: "key")
                }
                .disabled(engine.cloudBusy)
            }

            HStack {
                SecureField(engine.cloudHasPassphrase ? "Passphrase saved on this phone" : "Passphrase, at least \(CloudCrypto.minPassphrase) characters",
                            text: $passphrase)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                Button("Save") {
                    if engine.setPassphrase(passphrase) { passphrase = "" }
                }
                .disabled(passphrase.trimmingCharacters(in: .whitespaces).isEmpty)
                .buttonStyle(.borderless)
            }

            Toggle("Keep my library in the cloud", isOn: $engine.settings.cloudSync)

            HStack {
                Button {
                    Task { await engine.syncNow() }
                } label: {
                    Label("Sync now", systemImage: "arrow.triangle.2.circlepath")
                }
                .disabled(engine.cloudBusy || !engine.cloudRegistered || !engine.cloudHasPassphrase)
                if engine.cloudBusy { Spacer(); ProgressView() }
            }
            if !engine.cloudStatus.isEmpty {
                Text(engine.cloudStatus).font(.caption).foregroundStyle(Theme.good)
            }
            if !engine.cloudError.isEmpty {
                Text(engine.cloudError).font(.caption).foregroundStyle(Theme.bad)
            }

            DisclosureGroup("Firebase details", isExpanded: $engine.showFirebaseDetails) {
                TextField("Project ID", text: $engine.settings.firebaseProjectId)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                TextField("Web API key", text: $engine.settings.firebaseApiKey)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                TextField("Google iOS client ID — only to sign in", text: $engine.settings.googleClientId)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                googleWarning
            }
        } header: {
            Text("Account")
        } footer: {
            Text("Only needed to share your library with your Mac and other devices. It's encrypted on this phone first, with a passphrase that never leaves it. Use the same passphrase everywhere; there's no way to recover it.")
        }
    }

    private var googleWarning: some View {
        Label(Cloud.googleWarning, systemImage: "exclamationmark.triangle")
            .font(.caption)
            .foregroundStyle(Theme.warn)
    }

    // MARK: library

    private var librarySection: some View {
        Section {
            Button {
                exporting = true
            } label: {
                Label("Export library…", systemImage: "square.and.arrow.up")
            }
            Button {
                importing = true
            } label: {
                Label("Import library…", systemImage: "square.and.arrow.down")
            }
            if !libraryStatus.isEmpty {
                Text(libraryStatus).font(.caption).foregroundStyle(Theme.ink2)
            }
        } header: {
            Text("Library")
        } footer: {
            Text("Your words, dictionary, rules and history in one file. The Mac app reads it too, and importing only ever adds.")
        }
        .fileExporter(isPresented: $exporting, document: LibraryFile(library: engine.library),
                      contentType: .json, defaultFilename: "yap-library.json") { result in
            if case .failure(let error) = result { libraryStatus = error.localizedDescription } else { libraryStatus = "Exported" }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.json]) { result in
            switch result {
            case .success(let url):
                let scoped = url.startAccessingSecurityScopedResource()
                defer { if scoped { url.stopAccessingSecurityScopedResource() } }
                if let data = try? Data(contentsOf: url) {
                    libraryStatus = engine.importLibrary(data)
                } else {
                    libraryStatus = "Couldn't read that file."
                }
            case .failure(let error):
                libraryStatus = error.localizedDescription
            }
        }
    }

    @ViewBuilder
    private func modelRow(_ model: Transcriber.Model) -> some View {
        let status = engine.transcriber.status[model.id] ?? .notDownloaded
        let selected = engine.settings.model == model.id
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(selected ? Theme.accent : Theme.ink3)
                .font(.title3)
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(model.label).font(.body.weight(.semibold))
                    if !model.badge.isEmpty { Pill(text: model.badge, fg: Theme.accent, bg: Theme.accentSoft) }
                }
                Text("\(model.note) \(model.sizeMB) MB.").font(.caption).foregroundStyle(Theme.ink2)
                switch status {
                case .downloading(let fraction):
                    ProgressView(value: fraction).padding(.top, 4)
                case .failed(let message):
                    Text(message).font(.caption).foregroundStyle(Theme.bad)
                default:
                    EmptyView()
                }
            }
            Spacer()
            switch status {
            case .ready:
                Text("Ready").font(.caption.weight(.semibold)).foregroundStyle(Theme.good)
            case .downloading(let fraction):
                Text("\(Int(fraction * 100))%").font(.caption).foregroundStyle(Theme.ink3)
            default:
                Button {
                    Task { await engine.transcriber.download(model.id) }
                } label: {
                    Image(systemName: "arrow.down.circle.fill").font(.title2)
                }
                .buttonStyle(.plain)
                .foregroundStyle(Theme.accent)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { engine.settings.model = model.id }
        .swipeActions {
            if status == .ready {
                Button("Remove", role: .destructive) { engine.transcriber.delete(model.id) }
            }
        }
    }
}

/// The library as a file for the share sheet: `yap-library.json`, which the Mac imports too.
struct LibraryFile: FileDocument {
    static var readableContentTypes: [UTType] { [.json] }
    let library: Library

    init(library: Library) { self.library = library }

    init(configuration: ReadConfiguration) throws {
        guard let data = configuration.file.regularFileContents else { throw CocoaError(.fileReadCorruptFile) }
        library = try LibrarySync.read(data)
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: try LibrarySync.write(library))
    }
}
