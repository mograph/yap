import SwiftUI

struct TalkView: View {
    @EnvironmentObject var engine: Engine
    @State private var copiedId: String?

    /// Right after a dictation, the newest one is shown big on its own, so Recent starts after it.
    private var showsLatest: Bool { engine.phase == .done && !engine.history.isEmpty }
    private var recent: ArraySlice<Dictation> { engine.history.prefix(50).dropFirst(showsLatest ? 1 : 0) }

    private var needsModel: Bool {
        !engine.transcriber.isInstalled(engine.settings.model)
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 18) {
                    if needsModel { downloadBanner }
                    if engine.fromKeyboard && engine.phase == .listening { keyboardBanner }
                    micArea
                    if showsLatest, let latest = engine.history.first {
                        DictationRow(item: latest, highlighted: true)
                    }
                    if !engine.history.isEmpty {
                        VStack(alignment: .leading, spacing: 10) {
                            if !recent.isEmpty { Text("Recent").font(.system(.headline, design: .rounded)) }
                            ForEach(recent) { item in
                                DictationRow(item: item)
                                    .contextMenu {
                                        Button("Copy", systemImage: "doc.on.doc") { UIPasteboard.general.string = item.text }
                                        Button("Delete", systemImage: "trash", role: .destructive) { engine.delete(item) }
                                    }
                            }
                        }
                    } else {
                        emptyState
                    }
                }
                .padding(.horizontal, 18)
                .padding(.bottom, 30)
            }
            .background(Theme.paper)
            .toolbar {
                ToolbarItem(placement: .principal) {
                    HStack(spacing: 7) {
                        BrandMark(size: 26, live: engine.phase == .listening)
                        Text("yap").font(.system(size: 22, weight: .heavy, design: .rounded))
                    }
                }
                if engine.sessionLive {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("End", systemImage: "mic.slash") { engine.endSession() }
                            .labelStyle(.titleAndIcon)
                            .font(.footnote)
                    }
                }
            }
            .navigationBarTitleDisplayMode(.inline)
        }
    }

    private var micArea: some View {
        VStack(spacing: 14) {
            Button(action: engine.toggle) {
                ZStack {
                    Circle()
                        .fill(Theme.accent.opacity(0.14))
                        .frame(width: 190, height: 190)
                        .scaleEffect(engine.phase == .listening ? 1 + CGFloat(min(engine.level * 6, 0.35)) : 0.9)
                        .animation(.easeOut(duration: 0.12), value: engine.level)
                    Circle()
                        .fill(engine.phase == .listening ? Theme.accent : Theme.ink)
                        .frame(width: 128, height: 128)
                        .shadow(color: .black.opacity(0.2), radius: 16, y: 8)
                    Group {
                        if case .thinking = engine.phase {
                            ProgressView().tint(Theme.paper).scaleEffect(1.4)
                        } else {
                            Image(systemName: engine.phase == .listening ? "stop.fill" : "mic.fill")
                                .font(.system(size: 44, weight: .semibold))
                        }
                    }
                    .foregroundStyle(engine.phase == .listening ? .white : Theme.paper)
                }
            }
            .buttonStyle(.plain)
            .disabled(needsModel)
            .padding(.top, 12)

            if engine.phase == .done, let latest = engine.history.first {
                // What you said, with a check, instead of the word "Copied".
                Label {
                    Text(latest.text).lineLimit(3).multilineTextAlignment(.leading)
                } icon: {
                    Image(systemName: "checkmark.circle.fill").foregroundStyle(Theme.good)
                }
                .font(.system(.subheadline, design: .rounded).weight(.medium))
                .foregroundStyle(Theme.ink)
                .frame(minHeight: 40)
            } else {
                Text(statusText)
                    .font(.system(.subheadline, design: .rounded).weight(.medium))
                    .foregroundStyle(isError ? Theme.bad : Theme.ink2)
                    .multilineTextAlignment(.center)
                    .frame(minHeight: 40)
            }
        }
    }

    private var isError: Bool {
        if case .error = engine.phase { return true }
        return false
    }

    private var statusText: String {
        switch engine.phase {
        case .idle: return needsModel ? "Download a Whisper model to start" : "Tap and talk. Tap again when you're done."
        case .listening: return "Listening…"
        case .thinking(let message): return message
        case .done: return ""
        case .error(let message): return message
        }
    }

    private var downloadBanner: some View {
        let model = Transcriber.models.first { $0.id == engine.settings.model } ?? Transcriber.models[0]
        let status = engine.transcriber.status[model.id] ?? .notDownloaded
        return VStack(alignment: .leading, spacing: 10) {
            Text("Download \(model.label)").font(.system(.headline, design: .rounded))
            Text("Runs right on your iPhone. Free, private, works offline. \(model.sizeMB) MB, one time. Keep Yap open while it downloads.")
                .font(.subheadline).foregroundStyle(Theme.ink2)
            switch status {
            case .downloading(let fraction):
                ProgressView(value: fraction)
                Text("\(Int(fraction * 100))%").font(.caption).foregroundStyle(Theme.ink3)
            case .failed(let message):
                Text(message).font(.caption).foregroundStyle(Theme.bad)
                downloadButton(model.id)
            default:
                downloadButton(model.id)
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Theme.card))
    }

    private func downloadButton(_ id: String) -> some View {
        Button {
            Task { await engine.transcriber.download(id) }
        } label: {
            Label("Download", systemImage: "arrow.down.circle.fill").frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .controlSize(.large)
    }

    private var keyboardBanner: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: "keyboard.fill").foregroundStyle(Theme.accent)
            VStack(alignment: .leading, spacing: 4) {
                Text("Listening for your keyboard").font(.system(.subheadline, design: .rounded).weight(.semibold))
                Text("Go back to your app (tap ◀ in the top-left corner), keep talking, then tap ■ on the Yap keyboard. Yap stays ready for 5 minutes.")
                    .font(.footnote).foregroundStyle(Theme.ink2)
            }
        }
        .padding(14)
        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Theme.accentSoft))
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Text("Nothing yet. Say something.").font(.system(.headline, design: .rounded))
            Text("Everything you dictate shows up here, with exactly what Yap changed.")
                .font(.subheadline).foregroundStyle(Theme.ink2).multilineTextAlignment(.center)
        }
        .padding(.top, 20)
    }
}
