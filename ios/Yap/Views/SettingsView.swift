import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var engine: Engine
    @State private var confirmClear = false

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
                        Label("Set up the Yap keyboard", systemImage: "keyboard")
                    }
                    if engine.sessionLive {
                        Button("Turn off the mic now", systemImage: "mic.slash", role: .destructive) { engine.endSession() }
                    }
                } footer: {
                    Text("After you use the keyboard, the mic stays ready for 5 minutes so you don't have to jump to Yap again.")
                }

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
                         : "Removes um/uh and stutters, follows “bullet point” and “new line”, capitalizes. Works offline.")
                }

                Section {
                    Toggle("Haptics", isOn: $engine.settings.haptics)
                    Button(confirmClear ? "Tap again to clear \(engine.history.count) dictations" : "Clear history", role: .destructive) {
                        if confirmClear {
                            engine.history.removeAll()
                            engine.saveHistory()
                            confirmClear = false
                        } else {
                            confirmClear = true
                        }
                    }
                } footer: {
                    Text("Your history and settings stay on this iPhone.")
                }
            }
            .navigationTitle("Settings")
            .onAppear { engine.transcriber.refresh() }
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
