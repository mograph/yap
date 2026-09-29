import Foundation

// The Swift half of scripts/ios-parity.sh: same sentences, same fields, as the Rust `parity_dump`.
let args = CommandLine.arguments
let said = try! JSONDecoder().decode([String].self, from: Data(contentsOf: URL(fileURLWithPath: args[1])))
let profile = Profile()
let rows: [[String: Any]] = said.map { s in
    let clean = LocalRules.run(profile, s).text
    return [
        "input": s,
        "clean": clean,
        "list": Lists.asList(clean),
        "groups": Lists.asGroups(clean),
        "listDirect": Lists.asList(s),
        "groupsDirect": Lists.asGroups(s),
        "words": LocalRules.words(clean).count,
        "keptPct": String(format: "%.2f", LocalRules.keptPct(s, clean)),
    ]
}
let data = try! JSONSerialization.data(withJSONObject: rows, options: [.prettyPrinted, .sortedKeys])
try! data.write(to: URL(fileURLWithPath: args[2]))

// What Claude is told about the speaker.
if args.count > 3 {
    let profiles = try! JSONDecoder().decode([Profile].self, from: Data(contentsOf: URL(fileURLWithPath: args[3])))
    let out = profiles.map(Polish.speaker)
    try! JSONSerialization.data(withJSONObject: out, options: [.prettyPrinted]).write(to: URL(fileURLWithPath: args[3] + ".swift"))
}
