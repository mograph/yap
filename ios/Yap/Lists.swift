import Foundation

/// The tidier arrangements Moonshot can offer: a bullet list when you ran through several things
/// (a brain dump included), or your subjects gathered up when you bounced between them.
///
/// A line-for-line port of `as_list` and `as_groups` in src-tauri/src/polish.rs. The two have to
/// agree, so the patterns are copied verbatim and `scripts/ios-parity.sh` runs both over the
/// same sentences and fails on any difference. Change one, change both.
enum Lists {
    // MARK: patterns

    static let sentence = Pattern(#"[.!?]+\s+"#)
    /// Commas, plus the "and" / "or" / "then" / "also" that usually follow them in a spoken list.
    static let series = Pattern(#"(?i),\s*(?:and\s+(?:then\s+|also\s+)?|or\s+(?:also\s+)?|then\s+|also\s+|plus\s+)?"#)
    /// Sentence openers that mean "here starts the next item".
    static let cue = Pattern(#"""
        (?ix)
        ^ (?: (?: and | so | ok | okay | but | oh ) \s+ ){0,2}
        (?:
            (?: first (?:ly)? | second (?:ly)? | third (?:ly)? | fourth (?:ly)? | fifth (?:ly)?
              | next | then | also | plus | finally | lastly
              | number \s+ (?: one | two | three | four | five | six | seven ) )
            (?: \s+ (?: thing | one | up | point | item ) )?
          | (?: another | one \s+ more | the \s+ other | the \s+ last | last )
            \s+ (?: thing | one | point | item )
        )
        \b [,:]? \s*
        """#)
    /// "I'm gonna make a list", "I wanna do these tasks", "let me do a brain dump" — the clause
    /// that announces a list, so it belongs on the lead-in line, not in a bullet.
    static let announce = Pattern(#"""
        (?ix)
        \b(?:
            (?: make | makin[g'] | do | doing | give | giving | got | have
              | run (?:ning)? \s+ through | go (?:ing)? \s+ through
              | list | listing | cover | covering | walk \s+ through | talk \s+ about )
            \s+ (?: \w+ \s+ ){0,3}
          | here (?: '?s | \s+ is | \s+ are ) \s+ (?: \w+ \s+ ){0,3}
          | (?: a | two | three | four | five | couple | few ) \s+ (?: \w+ \s+ ){0,2}
        )
        (?: lists? | tasks | to-?dos? | things | items | points | notes | thoughts
          | feedback | updates | questions | steps | ideas | changes
          | brain\s*dump | dump | rundown | run-?through | round-?up )\b
        """#)
    /// An announcement is the speaker saying what's coming. Without one of these it's just an
    /// item that happens to mention a list ("Give Sam the meeting notes").
    static let announcer = Pattern(#"(?i)\b(?:i|i'?m|i'?ve|i'?ll|we|we'?re|let'?s|let\s+me|lemme|here|here'?s|these|following)\b"#)
    /// A bare conjunction at the front of an item: how it was spoken, not part of the thing.
    static let leadingJoin = Pattern(#"(?i)^(?:and|also|plus|then|so|but|okay|ok|oh|well|yeah)\s+(?:then\s+|also\s+)?"#)
    /// A piece that's only spoken filler. Not an item.
    static let fillerOnly = Pattern(#"(?i)^(?:u+m+|u+h+|e+r+|a+h+|oh|hmm+|like|so|then|also|and|but|or|plus|first|next|finally|okay|ok|alright|well|right|yeah|yep|hey|hi|anyway|honestly|seriously|literally|actually|basically|you know|i mean|i guess|kind of|sort of|let's see|let me think)$"#)
    /// A piece that can't start an item, so it's the tail of the one before it.
    static let continues = Pattern(#"(?i)^(?:which|who|whom|whose|that|because|'?cause|since|so|so that|but|though|although|whereas|rather than|instead|especially|mainly|ideally|basically|you know|i mean)\b"#)
    /// A comma-split piece opening with a preposition or subordinator: part of the clause
    /// beside it, not an item of its own.
    static let fragment = Pattern(#"(?i)^(?:if|when|where|unless|while|as|or|nor|not|for|from|with|about|in|on|at|by|to|after|before|during|without|like|maybe|even|just|only|at least|kind of|sort of)\b"#)
    /// Words a finished thought doesn't end on. Left out on purpose: "in", "up", "for", "that"
    /// and friends end clauses all the time ("the second person comes out or in").
    static let dangling = Pattern(#"(?i)\b(?:the|a|an|and|or|but|of|to|like|is|are|was|were|be|been|it'?s|that'?s|there'?s|you'?re|i'?m|we'?re|they'?re|he'?s|she'?s)$"#)

    // MARK: small helpers, matching Rust's trim family

    static func trim(_ s: String) -> String { s.trimmingCharacters(in: .whitespacesAndNewlines) }

    static func trimEnd(_ s: String, _ chars: String) -> String {
        var out = Substring(s)
        while let last = out.last, chars.contains(last) { out = out.dropLast() }
        return String(out)
    }

    static func trimStart(_ s: String, _ chars: String) -> String {
        var out = Substring(s)
        while let first = out.first, chars.contains(first) { out = out.dropFirst() }
        return String(out)
    }

    static func wordCount(_ s: String) -> Int { s.split(whereSeparator: \.isWhitespace).count }

    /// Does this piece announce the list that follows, rather than being part of it?
    static func announces(_ part: String) -> Bool { announce.matches(part) && announcer.matches(part) }

    static func bullet(_ item: String) -> String {
        let t = trim(trimEnd(trim(item), ".,;"))
        guard let first = t.first else { return "" }
        return "- " + String(first).uppercased() + t.dropFirst()
    }

    /// Splits a spoken series on its commas, and says whether the last gap carried a
    /// conjunction — the "and" before the final item that makes it a series.
    static func seriesPieces(_ body: String) -> (pieces: [String], tailJoined: Bool) {
        let ns = body as NSString
        var pieces: [String] = []
        var at = 0
        var tailJoined = false
        for gap in series.ranges(in: body) {
            pieces.append(trim(ns.substring(with: NSRange(location: at, length: gap.location - at))))
            tailJoined = !ns.substring(with: gap).trimmingCharacters(in: CharacterSet(charactersIn: ", ")).isEmpty
            at = gap.location + gap.length
        }
        pieces.append(trim(ns.substring(from: at)))
        return (pieces, tailJoined)
    }

    /// Sentences, each keeping its own end punctuation.
    static func sentencesWithEnds(_ body: String) -> [String] {
        let ns = body as NSString
        var out: [String] = []
        var at = 0
        for gap in sentence.ranges(in: body) {
            let gapText = ns.substring(with: gap)
            let punctuation = gapText.prefix { !$0.isWhitespace }
            let end = gap.location + (String(punctuation) as NSString).length
            out.append(trim(ns.substring(with: NSRange(location: at, length: end - at))))
            at = gap.location + gap.length
        }
        out.append(trim(ns.substring(from: at)))
        return out.filter { !$0.isEmpty }
    }

    // MARK: lists

    /// A bullet-list version of a dictation that runs through several things, or "" if it
    /// doesn't look like one.
    static func asList(_ input: String) -> String {
        let text = trim(input)
        if text.isEmpty || text.contains("\n- ") { return "" }

        // "Groceries: milk, eggs, and bread" keeps "Groceries:" as a lead-in (but not "10:30").
        var lead: [String] = []
        var body = text
        if let colon = text.firstIndex(of: ":") {
            let l = String(text[..<colon])
            let endsInDigit = l.unicodeScalars.last.map { ("0"..."9").contains($0) } ?? false
            if !l.contains(","), wordCount(l) <= 8, !endsInDigit {
                lead = [trim(l)]
                body = trim(String(text[text.index(after: colon)...]))
            }
        }

        let sentences = sentencesWithEnds(body)
        // The cue marks where an item starts; what follows it belongs to that item.
        let cued = sentences.count >= 3 && sentences.filter { cue.matches($0) }.count >= 2
        // No cues, but they said they were making a list and then took a sentence per thing.
        let perSentence = !cued && sentences.count >= 4 && (sentences.first.map(announces) ?? false)
        var tailJoined = false
        var parts: [String]
        if cued {
            var acc: [String] = []
            for line in sentences {
                // A lead-in never swallows the first item, and an uncued sentence continues the
                // item it was said as part of.
                if cue.matches(line) || (acc.last.map(announces) ?? true) {
                    acc.append(cue.replaceFirst(in: line, with: ""))
                } else {
                    acc[acc.count - 1] += " " + line
                }
            }
            parts = acc
        } else if perSentence {
            parts = sentences
        } else if sentences.count == 1 {
            (parts, tailJoined) = seriesPieces(body)
        } else {
            return ""
        }

        // Everything up to the actual items is the lead-in, while at least three items are left.
        while parts.count > 3 && announces(parts[0]) {
            lead.append(parts.removeFirst())
        }

        // One bullet per thing they said, not per comma: drop the "um"s, and fold a piece that
        // can't stand on its own back into the item it belongs to.
        var items: [String] = []
        for raw in parts {
            let part = trim(trimStart(trim(raw), ",;"))
            if part.isEmpty || fillerOnly.matches(trimEnd(part, ".,;")) { continue }
            let joinsPrevious = continues.matches(part) || (!cued && !perSentence && fragment.matches(part))
            if joinsPrevious {
                // Nothing to attach to: the whole thing is one clause that started mid-thought.
                if items.isEmpty { return "" }
                items[items.count - 1] += ", " + part
            } else {
                items.append(part)
            }
        }

        // Real items are parallel. A one-word scrap beside a full clause means the commas were
        // cut in the wrong places.
        let counts = items.map(wordCount)
        if (counts.min() ?? 0) <= 1 && (counts.max() ?? 0) >= 5 { return "" }
        // An item that trails off mid-sentence means the split landed inside a thought.
        if items.contains(where: { dangling.matches(trim(trimEnd($0, ".,;?!"))) }) { return "" }

        // Commas alone mean nothing: people talk in commas. Only a real series or a lead-in that
        // said a list was coming is evidence of one.
        if !cued && !perSentence {
            let listy = items.count >= 3 && (tailJoined || !lead.isEmpty) && items.allSatisfy { wordCount($0) <= 14 }
            if !listy { return "" }
        }
        let bullets = items.map { bullet(trim(leadingJoin.replaceFirst(in: $0, with: ""))) }.filter { !$0.isEmpty }
        if bullets.count < 3 { return "" }
        if lead.isEmpty { return bullets.joined(separator: "\n") }
        return trimEnd(lead.joined(separator: ", "), ",.;:") + ":\n" + bullets.joined(separator: "\n")
    }

    // MARK: subjects

    /// Words that say nothing about what a sentence is about.
    static let stop: Set<String> = [
        "about", "actually", "after", "again", "also", "another", "anything", "back", "because", "been", "before",
        "being", "bunch", "come", "could", "couple", "days", "different", "does", "doing", "done", "down", "else",
        "even", "ever", "every", "from", "gonna", "gotta", "guess", "have", "here", "into", "just", "kind", "know",
        "like", "little", "look", "lots", "make", "makes", "many", "mean", "more", "most", "much", "need", "never",
        "next", "nothing", "okay", "only", "other", "over", "part", "people", "pretty", "probably", "put", "really",
        "right", "said", "same", "say", "says", "should", "some", "something", "sort", "still", "stuff", "such",
        "sure", "take", "tell", "than", "that", "their", "them", "then", "there", "these", "they", "thing", "things",
        "think", "this", "those", "though", "through", "time", "times", "too", "took", "very", "want", "wanna",
        "was", "way", "well", "were", "what", "when", "where", "which", "while", "will", "with", "would", "yeah",
        "your", "youre",
    ]

    /// The words in a sentence that say what it's about, crudely de-pluralized.
    static func subjectWords(_ sentence: String) -> Set<String> {
        var out = Set<String>()
        for piece in sentence.split(whereSeparator: { !($0.isLetter || $0.isNumber) }) {
            let w = piece.lowercased()
            guard w.unicodeScalars.count >= 4, !stop.contains(w) else { continue }
            if w.hasSuffix("s") {
                let stem = String(w.dropLast())
                if stem.unicodeScalars.count >= 4 && !stem.hasSuffix("s") {
                    out.insert(stem)
                    continue
                }
            }
            out.insert(w)
        }
        return out
    }

    /// When someone rambles across two subjects and keeps switching back, gathers each subject's
    /// sentences into its own paragraph, in the order the subjects first came up. Nothing is
    /// dropped or reworded. "" unless the subjects genuinely interleave.
    static func asGroups(_ input: String) -> String {
        let text = trim(input)
        if text.contains("\n") { return "" }
        let sentences = sentencesWithEnds(text)
        if sentences.count < 4 { return "" }

        // Each sentence joins the subject it shares the most words with, or starts a new one.
        var topics: [(seen: Set<String>, at: [Int])] = []
        for (i, sentence) in sentences.enumerated() {
            let words = subjectWords(sentence)
            var best: (topic: Int, shared: Int)?
            for (t, topic) in topics.enumerated() {
                let shared = words.filter { topic.seen.contains($0) }.count
                // `>=` hands a tie to the later subject, as Rust's `max_by_key` does.
                if best == nil || shared >= best!.shared { best = (t, shared) }
            }
            if let best, best.shared > 0 {
                topics[best.topic].seen.formUnion(words)
                topics[best.topic].at.append(i)
            } else {
                topics.append((words, [i]))
            }
        }

        let switched = topics.contains { topic in zip(topic.at, topic.at.dropFirst()).contains { $1 != $0 + 1 } }
        if !(2...4).contains(topics.count) || topics.contains(where: { $0.at.count < 2 }) || !switched { return "" }
        return topics.map { $0.at.map { sentences[$0] }.joined(separator: " ") }.joined(separator: "\n\n")
    }

    /// A list when there is one, otherwise the subjects gathered up, otherwise nothing.
    static func tidier(_ text: String) -> String {
        let listed = asList(text)
        return listed.isEmpty ? asGroups(text) : listed
    }
}

/// A compiled pattern with the few operations the list logic needs. NSRegularExpression counts
/// in UTF-16, so everything goes through NSString to keep ranges honest.
struct Pattern {
    let re: NSRegularExpression

    init(_ pattern: String) {
        re = try! NSRegularExpression(pattern: pattern)
    }

    private func all(_ s: String) -> NSRange { NSRange(location: 0, length: (s as NSString).length) }

    func matches(_ s: String) -> Bool { re.firstMatch(in: s, range: all(s)) != nil }

    func ranges(in s: String) -> [NSRange] { re.matches(in: s, range: all(s)).map(\.range) }

    /// Like Rust's `Regex::replace`: the first match only.
    func replaceFirst(in s: String, with replacement: String) -> String {
        guard let m = re.firstMatch(in: s, range: all(s)) else { return s }
        return (s as NSString).replacingCharacters(in: m.range, with: replacement)
    }
}
