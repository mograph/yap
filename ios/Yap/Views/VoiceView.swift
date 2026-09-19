import SwiftUI

struct VoiceView: View {
    @EnvironmentObject var engine: Engine
    @State private var newWord = ""

    private static let kinds: [(id: String, label: String, blurb: String)] = [
        ("filler", "Filler words", "um, uh, and “like” when it's just padding"),
        ("correction", "Self-corrections", "stutters, repeats, “no wait, I mean…”"),
        ("punctuation", "Punctuation & caps", "sentence breaks, commas, question marks"),
        ("grammar", "Grammar", "agreement, tense, little missing words"),
        ("slang", "Slang", "turning casual words into standard ones"),
        ("rephrase", "Rewording", "rephrasing or reordering for clarity"),
        ("swearing", "Swearing", "softening or dropping profanity"),
        ("formatting", "Formatting", "bullet lists when you run through things"),
    ]

    private var tier: (name: String, example: String) {
        switch engine.profile.tone {
        case ...15: return ("Raw", "so yeah i'm gonna be like ten minutes late, traffic is kinda insane rn lol")
        case ...40: return ("Casual", "So yeah, I'm gonna be like ten minutes late, traffic is kinda insane rn lol")
        case ...65: return ("Relaxed", "So yeah, I'm gonna be about ten minutes late. Traffic is kinda insane right now, lol.")
        case ...85: return ("Tidy", "I'm going to be about ten minutes late. Traffic is pretty bad right now.")
        default: return ("Polished", "I'll be about ten minutes late; traffic is heavy at the moment.")
        }
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    HStack {
                        Text("How polished?").font(.system(.headline, design: .rounded))
                        Spacer()
                        Pill(text: tier.name, fg: Theme.accent, bg: Theme.accentSoft)
                    }
                    Slider(value: Binding(get: { Double(engine.profile.tone) }, set: { engine.profile.tone = Int($0) }), in: 0...100)
                    HStack {
                        Text("exactly how I talk")
                        Spacer()
                        Text("polished")
                    }
                    .font(.caption).foregroundStyle(Theme.ink3)
                    Text(tier.example)
                        .font(.subheadline)
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: 10).fill(Theme.goodSoft))
                }

                Section {
                    ForEach(Self.kinds, id: \.id) { kind in
                        VStack(alignment: .leading, spacing: 8) {
                            Text(kind.label).font(.body.weight(.semibold))
                            Text(kind.blurb).font(.caption).foregroundStyle(Theme.ink2)
                            Picker(kind.label, selection: rule(kind.id)) {
                                Text("Change it").tag(Rule.change)
                                Text("Suggest").tag(Rule.suggest)
                                Text("Leave it").tag(Rule.leave)
                            }
                            .pickerStyle(.segmented)
                            .labelsHidden()
                        }
                        .padding(.vertical, 4)
                    }
                } header: {
                    Text("What Yap can change")
                } footer: {
                    Text("Suggestions show up under “What changed” without touching your text.")
                }

                Section {
                    ForEach(engine.profile.myWords, id: \.self) { Text($0) }
                        .onDelete { engine.profile.myWords.remove(atOffsets: $0) }
                    HStack {
                        TextField("Add a word, like lowkey", text: $newWord)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .onSubmit(addWord)
                        Button("Add", action: addWord).disabled(newWord.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                } header: {
                    Text("Words that are mine")
                } footer: {
                    Text("Slang and spellings Yap never “fixes”.")
                }

                Section {
                    ForEach($engine.profile.dictionary, id: \.self) { $entry in
                        HStack {
                            TextField("when I say…", text: $entry.say).textInputAutocapitalization(.never)
                            Image(systemName: "arrow.right").foregroundStyle(Theme.ink3)
                            TextField("write…", text: $entry.write)
                        }
                    }
                    .onDelete { engine.profile.dictionary.remove(atOffsets: $0) }
                    Button("Add a word", systemImage: "plus") { engine.profile.dictionary.append(DictEntry()) }
                } header: {
                    Text("Say this, write that")
                } footer: {
                    Text("Names and brands. These also help Whisper hear them right.")
                }

                Section("About how you talk") {
                    TextField("What should Yap call you?", text: $engine.profile.name)
                    TextField("I never capitalize in texts, I say “like” a lot…", text: $engine.profile.notes, axis: .vertical)
                        .lineLimit(3...8)
                }

                Section {
                    TextField("Paste a few messages you've actually sent", text: $engine.profile.samples, axis: .vertical)
                        .lineLimit(4...12)
                } header: {
                    Text("Samples of your writing")
                } footer: {
                    Text("Claude matches the vibe.")
                }
            }
            .navigationTitle("Your voice")
        }
    }

    private func rule(_ kind: String) -> Binding<Rule> {
        Binding(get: { engine.profile.rule(kind) }, set: { engine.profile.rules[kind] = $0 })
    }

    private func addWord() {
        let word = newWord.trimmingCharacters(in: .whitespaces)
        guard !word.isEmpty, !engine.profile.myWords.contains(where: { $0.caseInsensitiveCompare(word) == .orderedSame }) else { return }
        engine.profile.myWords.append(word)
        newWord = ""
    }
}
