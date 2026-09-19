import SwiftUI

struct DictationRow: View {
    let item: Dictation
    var highlighted = false
    @State private var open = false
    @State private var copied = false

    private var applied: [Edit] { item.edits.filter(\.applied) }
    private var suggested: [Edit] { item.edits.filter { !$0.applied } }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 10) {
                Text(item.text)
                    .font(.system(size: 16))
                    .foregroundStyle(Theme.ink)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Button {
                    UIPasteboard.general.string = item.text
                    copied = true
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.4) { copied = false }
                } label: {
                    Image(systemName: copied ? "checkmark" : "doc.on.doc").frame(width: 30, height: 30)
                }
                .foregroundStyle(Theme.ink3)
            }

            HStack(spacing: 6) {
                Text(item.date, style: .relative).font(.caption).foregroundStyle(Theme.ink3)
                Pill(text: "\(Int(item.keptPct.rounded()))% you", fg: Theme.accent, bg: Theme.accentSoft)
                if !applied.isEmpty { Pill(text: "\(applied.count) change\(applied.count == 1 ? "" : "s")", fg: Theme.ink2, bg: Theme.card2) }
                if !suggested.isEmpty { Pill(text: "\(suggested.count) suggestion\(suggested.count == 1 ? "" : "s")", fg: Theme.warn, bg: Theme.warnSoft) }
                Spacer()
                if !item.edits.isEmpty || item.raw != item.text {
                    Button(open ? "Hide" : "What changed") { withAnimation(.snappy) { open.toggle() } }
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(Theme.ink2)
                }
            }

            if !item.note.isEmpty {
                Label(item.note, systemImage: "exclamationmark.triangle")
                    .font(.caption).foregroundStyle(Theme.warn)
                    .padding(8)
                    .background(RoundedRectangle(cornerRadius: 10).fill(Theme.warnSoft))
            }

            if open {
                VStack(alignment: .leading, spacing: 10) {
                    Text("YOU SAID").font(.caption2.weight(.semibold)).foregroundStyle(Theme.ink3)
                    Text(item.raw).font(.subheadline).foregroundStyle(Theme.ink2).textSelection(.enabled)
                    ForEach(Array(item.edits.enumerated()), id: \.offset) { _, e in
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            Text(e.kind.capitalized).font(.caption.weight(.semibold)).foregroundStyle(Theme.ink2).frame(width: 84, alignment: .leading)
                            Text(e.original).strikethrough(e.applied).foregroundStyle(e.applied ? Theme.bad : Theme.ink2)
                            Image(systemName: "arrow.right").font(.caption2).foregroundStyle(Theme.ink3)
                            Text(e.replacement.isEmpty ? (e.applied ? "removed" : "remove") : e.replacement)
                                .foregroundStyle(e.applied ? Theme.good : Theme.warn)
                        }
                        .font(.subheadline)
                    }
                }
                .padding(12)
                .background(RoundedRectangle(cornerRadius: 12).fill(Theme.card2))
            }
        }
        .padding(16)
        .background(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .fill(Theme.card)
                .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(highlighted ? Theme.accent.opacity(0.5) : Theme.line)))
    }
}

struct Pill: View {
    let text: String
    let fg: Color
    let bg: Color

    var body: some View {
        Text(text)
            .font(.system(size: 11, weight: .semibold, design: .rounded))
            .foregroundStyle(fg)
            .padding(.horizontal, 8).padding(.vertical, 3)
            .background(Capsule().fill(bg))
    }
}
