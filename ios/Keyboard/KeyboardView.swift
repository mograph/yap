import SwiftUI

struct KeyboardView: View {
    @ObservedObject var model: KeyboardModel
    let onMic: () -> Void
    let onSpace: () -> Void
    let onDelete: () -> Void
    let onReturn: () -> Void
    let onGlobe: () -> Void
    let onPasteLast: () -> Void
    let onOpenApp: () -> Void
    let onChoose: (Bool) -> Void

    @State private var pulse = false

    private var listening: Bool { model.state == "listening" }
    private var thinking: Bool { model.state == "thinking" }

    var body: some View {
        VStack(spacing: 10) {
            if model.needsFullAccess {
                fullAccessHint
            } else if let offer = model.offer {
                offerPanel(offer)
            } else {
                HStack(spacing: 8) {
                    Text(model.status)
                        .font(.system(size: 14, weight: .medium, design: .rounded))
                        .foregroundStyle(model.state == "error" ? Theme.bad : Theme.ink2)
                        .lineLimit(2)
                    Spacer(minLength: 4)
                    if !model.lastResult.isEmpty && !listening {
                        Button(action: onPasteLast) {
                            Label("Paste last", systemImage: "arrow.uturn.backward")
                                .font(.system(size: 13, weight: .semibold, design: .rounded))
                                .padding(.horizontal, 10).padding(.vertical, 6)
                                .background(Capsule().fill(Theme.card))
                        }
                        .foregroundStyle(Theme.ink)
                    }
                }
                .padding(.horizontal, 14)
                .frame(height: 30)

                micButton
            }

            HStack(spacing: 6) {
                if model.showsGlobe {
                    KeyButton(systemImage: "globe", width: 46, action: onGlobe)
                }
                KeyButton(label: "space", action: onSpace)
                KeyButton(systemImage: "delete.left", width: 58, action: onDelete)
                KeyButton(systemImage: "return", width: 70, action: onReturn)
            }
            .padding(.horizontal, 6)
        }
        .padding(.top, 8)
        .padding(.bottom, 6)
    }

    private var micButton: some View {
        Button(action: onMic) {
            ZStack {
                if listening {
                    Circle()
                        .stroke(Theme.accent.opacity(0.35), lineWidth: 6)
                        .frame(width: 104, height: 104)
                        .scaleEffect(pulse ? 1.15 : 0.95)
                        .opacity(pulse ? 0 : 1)
                        .animation(.easeOut(duration: 1.2).repeatForever(autoreverses: false), value: pulse)
                        .onAppear { pulse = true }
                        .onDisappear { pulse = false }
                }
                Circle()
                    .fill(listening ? Theme.accent : Theme.ink)
                    .frame(width: 84, height: 84)
                    .shadow(color: .black.opacity(0.18), radius: 10, y: 5)
                if thinking {
                    ProgressView().tint(Theme.paper)
                } else {
                    Image(systemName: listening ? "stop.fill" : "mic.fill")
                        .font(.system(size: listening ? 26 : 30, weight: .semibold))
                        .foregroundStyle(listening ? .white : Theme.paper)
                }
            }
            .frame(height: 108)
        }
        .buttonStyle(.plain)
        .disabled(thinking)
        .accessibilityLabel(listening ? "Stop and type" : "Talk")
    }

    /// "Make it a list?" with a peek at it, and a bar that runs out into "as said".
    private func offerPanel(_ offer: KeyboardModel.Offer) -> some View {
        let lines = offer.list.split(separator: "\n").map(String.init)
        let bulleted = lines.contains { $0.hasPrefix("- ") }
        return VStack(alignment: .leading, spacing: 8) {
            Text(bulleted ? "Make it a list?" : "Group it by subject?")
                .font(.system(size: 15, weight: .semibold, design: .rounded))
            VStack(alignment: .leading, spacing: 2) {
                ForEach(Array(lines.prefix(4).enumerated()), id: \.offset) { _, line in
                    Text(line.hasPrefix("- ") ? "• " + line.dropFirst(2) : line)
                        .font(.system(size: 13, design: .rounded))
                        .foregroundStyle(line.hasPrefix("- ") ? Theme.ink : Theme.ink2)
                        .lineLimit(1)
                }
                if lines.count > 4 {
                    Text("…and \(lines.count - 4) more").font(.system(size: 12, design: .rounded)).foregroundStyle(Theme.ink3)
                }
            }
            .padding(.horizontal, 10).padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Theme.card))
            HStack(spacing: 8) {
                Button { onChoose(false) } label: {
                    Text("As said").frame(maxWidth: .infinity).padding(.vertical, 9)
                        .background(Capsule().fill(Theme.card))
                }
                .foregroundStyle(Theme.ink)
                Button { onChoose(true) } label: {
                    Text(bulleted ? "As a list" : "Grouped").frame(maxWidth: .infinity).padding(.vertical, 9)
                        .background(Capsule().fill(Theme.accent))
                }
                .foregroundStyle(.white)
            }
            .font(.system(size: 14, weight: .semibold, design: .rounded))
            .buttonStyle(.plain)
            CountdownBar(started: offer.started, length: KeyboardModel.Offer.wait)
        }
        .padding(.horizontal, 12)
        .frame(height: 146, alignment: .top)
    }

    private var fullAccessHint: some View {
        VStack(spacing: 8) {
            Image(systemName: "hand.raised.fill").font(.title2).foregroundStyle(Theme.accent)
            Text("Moonshot needs Full Access to hear you")
                .font(.system(size: 15, weight: .semibold, design: .rounded))
            Text("Settings → General → Keyboard → Keyboards → Moonshot → Allow Full Access")
                .font(.system(size: 12.5, design: .rounded))
                .foregroundStyle(Theme.ink2)
                .multilineTextAlignment(.center)
            Button("Open Moonshot for help", action: onOpenApp)
                .font(.system(size: 13, weight: .semibold, design: .rounded))
                .foregroundStyle(Theme.accent)
        }
        .padding(.horizontal, 20)
        .frame(height: 146)
    }
}

private struct KeyButton: View {
    var label: String? = nil
    var systemImage: String? = nil
    var width: CGFloat? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Group {
                if let systemImage {
                    Image(systemName: systemImage).font(.system(size: 17, weight: .medium))
                } else if let label {
                    Text(label).font(.system(size: 15, weight: .regular))
                }
            }
            .frame(maxWidth: width == nil ? .infinity : width, minHeight: 44)
            .frame(width: width)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(Color(uiColor: .systemBackground).opacity(0.92))
                    .shadow(color: .black.opacity(0.25), radius: 0, y: 1))
            .foregroundStyle(Color(uiColor: .label))
        }
        .buttonStyle(.plain)
    }
}

/// Runs out over the offer's wait, so you can see when it'll type the words as said.
private struct CountdownBar: View {
    let started: Date
    let length: TimeInterval

    var body: some View {
        TimelineView(.animation) { context in
            let left = max(0, 1 - context.date.timeIntervalSince(started) / length)
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.line)
                    Capsule().fill(Theme.accent).frame(width: geo.size.width * left)
                }
            }
            .frame(height: 3)
        }
    }
}
