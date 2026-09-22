import SwiftUI

struct DictationRow: View {
    @EnvironmentObject var engine: Engine
    let item: Dictation
    var highlighted = false
    @State private var open = false
    @State private var copied = false

    private var applied: [Edit] { item.edits.filter(\.applied) }
    private var suggested: [Edit] { item.edits.filter { !$0.applied } }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 10) {
                FormattedText(text: item.text)
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

            // Its own row: squeezed in beside the pills it wrapped a letter at a time.
            if !item.list.isEmpty {
                Button { engine.takeList(item) } label: {
                    Label(item.list.contains("- ") ? "Make it a list" : "Group by subject", systemImage: "list.bullet")
                        .font(.subheadline.weight(.semibold))
                        .lineLimit(1)
                        .fixedSize()
                        .padding(.horizontal, 12).padding(.vertical, 7)
                        .background(Capsule().stroke(Theme.line))
                }
                .foregroundStyle(Theme.ink)
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

/// Dictated text, with bullet lines drawn as a real list instead of lines that start with a dash.
/// Same idea as `blocks()` on the Mac.
struct FormattedText: View {
    let text: String

    private var blocks: [(isList: Bool, lines: [String])] {
        var out: [(isList: Bool, lines: [String])] = []
        for line in text.components(separatedBy: "\n") {
            let bullet = line.trimmingCharacters(in: .whitespaces)
            let item = ["- ", "* ", "• "].first { bullet.hasPrefix($0) }.map { String(bullet.dropFirst($0.count)) }
            if let item {
                if out.last?.isList == true { out[out.count - 1].lines.append(item) } else { out.append((true, [item])) }
            } else if let last = out.last, !last.isList {
                out[out.count - 1].lines.append(line)
            } else {
                out.append((false, [line]))
            }
        }
        return out
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                if block.isList {
                    VStack(alignment: .leading, spacing: 5) {
                        ForEach(Array(block.lines.enumerated()), id: \.offset) { _, line in
                            HStack(alignment: .firstTextBaseline, spacing: 9) {
                                Circle().fill(Theme.accent).frame(width: 5, height: 5).alignmentGuide(.firstTextBaseline) { $0[.bottom] + 1 }
                                Text(line)
                            }
                        }
                    }
                } else {
                    Text(block.lines.joined(separator: "\n"))
                }
            }
        }
        .font(.system(size: 16))
        .foregroundStyle(Theme.ink)
        .textSelection(.enabled)
    }
}
