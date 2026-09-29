import Foundation

/// Folding one library into another. The same rules as src-tauri/src/library.rs, so the phone and
/// the Mac converge on the same answer whichever of them syncs first.
enum LibrarySync {
    /// Most dictations kept, newest first.
    static let limit = 2000

    /// Adds dictations you don't have yet and drops deleted ones. Returns how many were added.
    @discardableResult
    static func mergeHistory(_ into: inout [Dictation], _ from: [Dictation], deleted: [String]) -> Int {
        let gone = Set(deleted)
        into.removeAll { gone.contains($0.id) }
        var known = Set(into.map(\.id))
        var added = 0
        for d in from where !gone.contains(d.id) && known.insert(d.id).inserted {
            into.append(d)
            added += 1
        }
        // Same order as the Mac: by the timestamp string, newest first.
        into.sort { $0.createdAt > $1.createdAt }
        if into.count > limit { into.removeLast(into.count - limit) }
        return added
    }

    /// Import: adds words, dictionary entries, notes and samples you don't have. Never removes anything.
    @discardableResult
    static func addProfile(_ into: inout Profile, _ from: Profile) -> Int {
        var added = 0
        for word in from.myWords.map({ $0.trimmingCharacters(in: .whitespaces) }) where !word.isEmpty {
            if !into.myWords.contains(where: { $0.caseInsensitiveCompare(word) == .orderedSame }) {
                into.myWords.append(word)
                added += 1
            }
        }
        for entry in from.dictionary where !entry.say.trimmingCharacters(in: .whitespaces).isEmpty {
            let say = entry.say.trimmingCharacters(in: .whitespaces)
            if !into.dictionary.contains(where: { $0.say.trimmingCharacters(in: .whitespaces).caseInsensitiveCompare(say) == .orderedSame }) {
                into.dictionary.append(entry)
                added += 1
            }
        }
        for key in [\Profile.samples, \Profile.notes] {
            let theirs = from[keyPath: key].trimmingCharacters(in: .whitespacesAndNewlines)
            guard !theirs.isEmpty, !into[keyPath: key].contains(theirs) else { continue }
            if !into[keyPath: key].trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { into[keyPath: key] += "\n\n" }
            into[keyPath: key] += theirs
            added += 1
        }
        return added
    }

    /// Folds another device's library into this one. Profile: the newest edit wins. History and
    /// deletions: everything from both sides. Returns whether anything here changed, and the merged
    /// library to hand back to wherever the remote one came from.
    static func merge(_ remote: Library?, profile: inout Profile, history: inout [Dictation], deleted: inout [String]) -> (changed: Bool, merged: Library) {
        var changed = false
        let before = history.map(\.id)
        if let remote {
            for id in remote.deleted where !deleted.contains(id) {
                deleted.append(id)
                changed = true
            }
            if remote.profile.updatedAt > profile.updatedAt {
                profile = remote.profile
                changed = true
            }
            mergeHistory(&history, remote.history, deleted: deleted)
        } else {
            mergeHistory(&history, [], deleted: deleted)
        }
        if history.map(\.id) != before { changed = true }
        return (changed, Library(profile: profile, history: history, deleted: deleted))
    }

    /// True when the merged library says something the remote one didn't, so it's worth sending back.
    static func differs(_ remote: Library?, _ merged: Library) -> Bool {
        guard let remote else { return true }
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        return (try? encoder.encode(remote)) != (try? encoder.encode(merged))
    }

    /// Reads a library file exported from here or from the Mac.
    static func read(_ data: Data) throws -> Library {
        guard let lib = try? JSONDecoder().decode(Library.self, from: data), lib.app == "yap" else {
            throw Polish.Failure(message: "That file isn't a Yap library.")
        }
        return lib
    }

    static func write(_ lib: Library) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return try encoder.encode(lib)
    }
}
