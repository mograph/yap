import Charts
import SwiftUI

/// What Moonshot changes, what it only suggests, and how much of you makes it through. The same
/// numbers as the Mac's Insights (src/views/Insights.svelte), from the same history.
struct InsightsView: View {
    @EnvironmentObject var engine: Engine
    @State private var range = "30"

    private static let kindLabels: [String: String] = [
        "filler": "Filler words", "correction": "Self-corrections", "punctuation": "Punctuation & caps",
        "grammar": "Grammar", "slang": "Slang", "rephrase": "Rewording", "swearing": "Swearing",
        "formatting": "Formatting", "dictionary": "Your dictionary",
    ]

    private var list: [Dictation] {
        guard let days = Double(range) else { return engine.history }
        return engine.history.filter { Date().timeIntervalSince($0.date) < days * 86_400 }
    }

    private struct Totals {
        var count = 0, words = 0, kept = 0.0, savedMin = 0.0
    }

    private var totals: Totals {
        let l = list
        var t = Totals(count: l.count, words: l.reduce(0) { $0 + $1.words })
        t.kept = l.isEmpty ? 0 : l.reduce(0) { $0 + $1.keptPct } / Double(l.count)
        // Typing at ~40 wpm vs. how long you actually talked. Dictations that arrived without a
        // recorded length are skipped rather than counted as taking no time at all.
        t.savedMin = l.reduce(0) { $1.audioSecs > 0 ? $0 + max(0, Double($1.words) / 40 - $1.audioSecs / 60) : $0 }
        return t
    }

    private struct KindRow: Identifiable {
        let kind: String, changed: Int, suggested: Int
        var id: String { kind }
    }

    private var byKind: [KindRow] {
        var counts: [String: (Int, Int)] = [:]
        for d in list {
            for e in d.edits {
                var c = counts[e.kind, default: (0, 0)]
                if e.applied { c.0 += 1 } else { c.1 += 1 }
                counts[e.kind] = c
            }
        }
        return counts.map { KindRow(kind: Self.kindLabels[$0.key] ?? $0.key.capitalized, changed: $0.value.0, suggested: $0.value.1) }
            .sorted { $0.changed + $0.suggested > $1.changed + $1.suggested }
    }

    private struct Point: Identifiable {
        let at: Date, value: Double
        var id: Date { at }
    }

    /// A day per point while the range is short, then weeks and months, so an imported year of
    /// history shows up on the chart instead of stopping at 90 days.
    private var trend: (points: [Point], step: Int) {
        let l = list
        let oldest = l.map(\.date).min() ?? Date()
        let span = Double(range).map(Int.init) ?? max(7, Int(ceil(Date().timeIntervalSince(oldest) / 86_400)) + 1)
        let step = span <= 45 ? 1 : span <= 400 ? 7 : 30
        let calendar = Calendar.current
        let start = calendar.date(byAdding: .day, value: -(span - 1), to: calendar.startOfDay(for: Date()))!
        let buckets = Int(ceil(Double(span) / Double(step)))
        var sums = [Double](repeating: 0, count: buckets), counts = [Int](repeating: 0, count: buckets)
        for d in l {
            let i = Int(floor(d.date.timeIntervalSince(start) / (Double(step) * 86_400)))
            if i >= 0 && i < buckets {
                sums[i] += d.keptPct
                counts[i] += 1
            }
        }
        let points = (0..<buckets).compactMap { i -> Point? in
            guard counts[i] > 0 else { return nil }
            return Point(at: calendar.date(byAdding: .day, value: i * step, to: start)!, value: sums[i] / Double(counts[i]))
        }
        return (points, step)
    }

    private var fillers: [(word: String, count: Int)] {
        var counts: [String: Int] = [:]
        for d in list {
            for e in d.edits where e.kind == "filler" {
                let w = e.original.lowercased().filter { $0.isLetter || $0.isNumber || $0 == "'" || $0 == " " }
                    .trimmingCharacters(in: .whitespaces)
                if !w.isEmpty { counts[w, default: 0] += 1 }
            }
        }
        return counts.sorted { $0.value > $1.value }.prefix(8).map { ($0.key, $0.value) }
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Picker("Date range", selection: $range) {
                        Text("7 days").tag("7")
                        Text("30 days").tag("30")
                        Text("All time").tag("all")
                    }
                    .pickerStyle(.segmented)

                    if list.isEmpty {
                        card {
                            Text("No data for this range yet").font(.system(.headline, design: .rounded))
                            Text("Dictate a few things and this fills up with what Moonshot changed, what it held back, and how much still sounds like you.")
                                .font(.subheadline).foregroundStyle(Theme.ink2)
                        }
                    } else {
                        let t = totals
                        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
                            tile("Dictations", t.count.formatted(.number.notation(.compactName)))
                            tile("Words", t.words.formatted(.number.notation(.compactName)))
                            tile("Time saved", Self.minutes(t.savedMin), sub: "vs typing at 40 wpm")
                            tile("Still sounds like you", "\(Int(t.kept.rounded()))%", sub: "average words kept")
                        }

                        card {
                            Text("What Moonshot changed vs. what it could have").font(.system(.headline, design: .rounded))
                            Text("Changed ones it made; suggested ones it held back because your rules said so.")
                                .font(.caption).foregroundStyle(Theme.ink2)
                            if byKind.isEmpty {
                                Text("No edits yet. Moonshot left everything exactly as you said it.")
                                    .font(.subheadline).foregroundStyle(Theme.ink2)
                            } else {
                                Chart(byKind) { row in
                                    BarMark(x: .value("Edits", row.changed), y: .value("Kind", row.kind))
                                        .foregroundStyle(by: .value("", "Changed"))
                                    BarMark(x: .value("Edits", row.suggested), y: .value("Kind", row.kind))
                                        .foregroundStyle(by: .value("", "Suggested"))
                                }
                                .chartForegroundStyleScale(["Changed": Theme.accent, "Suggested": Theme.warn])
                                .frame(height: CGFloat(byKind.count) * 30 + 40)
                            }
                        }

                        card {
                            Text("How much still sounds like you").font(.system(.headline, design: .rounded))
                            Text("Average share of your own words (ignoring um and uh) that made it into the final text.")
                                .font(.caption).foregroundStyle(Theme.ink2)
                            let trend = trend
                            Chart(trend.points) { p in
                                LineMark(x: .value("When", p.at), y: .value("Kept", p.value))
                                    .foregroundStyle(Theme.accent)
                                PointMark(x: .value("When", p.at), y: .value("Kept", p.value))
                                    .foregroundStyle(Theme.accent)
                            }
                            .chartYScale(domain: 0...100)
                            .chartYAxis {
                                AxisMarks(values: [0, 25, 50, 75, 100]) { value in
                                    AxisGridLine()
                                    AxisValueLabel { Text("\(Int(value.as(Double.self) ?? 0))%") }
                                }
                            }
                            .frame(height: 180)
                        }

                        if !fillers.isEmpty {
                            card {
                                Text("Your fillers").font(.system(.headline, design: .rounded))
                                ForEach(fillers, id: \.word) { f in
                                    HStack {
                                        Text(f.word)
                                        Spacer()
                                        Text("\(f.count)").foregroundStyle(Theme.ink2).monospacedDigit()
                                    }
                                    .font(.subheadline)
                                }
                            }
                        }
                    }
                }
                .padding(18)
            }
            .background(Theme.paper)
            .navigationTitle("Insights")
        }
    }

    static func minutes(_ min: Double) -> String {
        if min < 1 { return "\(Int((min * 60).rounded()))s" }
        if min < 60 { return "\(Int(min.rounded())) min" }
        return String(format: "%.1f hr", min / 60)
    }

    private func tile(_ label: String, _ value: String, sub: String = "") -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label).font(.caption.weight(.semibold)).foregroundStyle(Theme.ink2)
            Text(value).font(.system(size: 26, weight: .bold, design: .rounded)).foregroundStyle(Theme.ink)
            if !sub.isEmpty { Text(sub).font(.caption2).foregroundStyle(Theme.ink3) }
        }
        .padding(14)
        .frame(maxWidth: .infinity, minHeight: 92, alignment: .topLeading)
        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Theme.card))
    }

    private func card<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8, content: content)
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Theme.card))
    }
}
