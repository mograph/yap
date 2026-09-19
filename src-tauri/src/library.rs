//! Your library: voice profile, history and deletions in one JSON file. Export and import it,
//! or keep it in a synced folder (Google Drive, iCloud, Synology…) so every computer shares it.

use crate::store::{Dictation, Profile};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub const FILE: &str = "yap-library.json";
const LIMIT: usize = 2000;

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Library {
    pub app: String,
    pub version: u32,
    pub profile: Profile,
    pub history: Vec<Dictation>,
    /// Dictations deleted on any computer, so syncing doesn't bring them back.
    pub deleted: Vec<String>,
}

impl Library {
    pub fn new(profile: &Profile, history: &[Dictation], deleted: &[String]) -> Self {
        Self {
            app: "yap".into(),
            version: 1,
            profile: profile.clone(),
            history: history.to_vec(),
            deleted: deleted.to_vec(),
        }
    }
}

pub fn read(path: &Path) -> Result<Library, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    let lib: Library = serde_json::from_str(&text).map_err(|_| "That file isn't a Yap library.".to_string())?;
    if lib.app != "yap" {
        return Err("That file isn't a Yap library.".into());
    }
    Ok(lib)
}

pub fn write(path: &Path, lib: &Library) -> Result<(), String> {
    let json = serde_json::to_string_pretty(lib).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Adds dictations you don't have yet and drops deleted ones. Returns how many were added.
pub fn merge_history(into: &mut Vec<Dictation>, from: &[Dictation], deleted: &[String]) -> usize {
    let gone: HashSet<&str> = deleted.iter().map(String::as_str).collect();
    into.retain(|d| !gone.contains(d.id.as_str()));
    let mut known: HashSet<String> = into.iter().map(|d| d.id.clone()).collect();
    let mut added = 0;
    for d in from {
        if !gone.contains(d.id.as_str()) && known.insert(d.id.clone()) {
            into.push(d.clone());
            added += 1;
        }
    }
    into.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    into.truncate(LIMIT);
    added
}

/// Import: adds words, dictionary entries, notes and samples you don't have. Never removes anything.
pub fn add_profile(into: &mut Profile, from: &Profile) -> usize {
    let mut added = 0;
    for word in from.my_words.iter().map(|w| w.trim()).filter(|w| !w.is_empty()) {
        if !into.my_words.iter().any(|m| m.eq_ignore_ascii_case(word)) {
            into.my_words.push(word.to_string());
            added += 1;
        }
    }
    for entry in from.dictionary.iter().filter(|e| !e.say.trim().is_empty()) {
        if !into.dictionary.iter().any(|m| m.say.trim().eq_ignore_ascii_case(entry.say.trim())) {
            into.dictionary.push(entry.clone());
            added += 1;
        }
    }
    for (mine, theirs) in [(&mut into.samples, &from.samples), (&mut into.notes, &from.notes)] {
        let theirs = theirs.trim();
        if !theirs.is_empty() && !mine.contains(theirs) {
            if !mine.trim().is_empty() {
                mine.push_str("\n\n");
            }
            mine.push_str(theirs);
            added += 1;
        }
    }
    added
}

/// Folds another computer's library into this one. Profile: the newest edit wins. History and
/// deletions: everything from both sides. Returns whether anything here changed, and the merged
/// library to hand back to wherever the remote one came from.
pub fn merge(
    remote: Option<&Library>,
    profile: &mut Profile,
    history: &mut Vec<Dictation>,
    deleted: &mut Vec<String>,
) -> (bool, Library) {
    let mut changed = false;
    let before: Vec<String> = history.iter().map(|d| d.id.clone()).collect();
    if let Some(remote) = remote {
        for id in &remote.deleted {
            if !deleted.contains(id) {
                deleted.push(id.clone());
                changed = true;
            }
        }
        if remote.profile.updated_at > profile.updated_at {
            *profile = remote.profile.clone();
            changed = true;
        }
        merge_history(history, &remote.history, deleted);
    } else {
        merge_history(history, &[], deleted);
    }
    if history.iter().map(|d| &d.id).ne(before.iter()) {
        changed = true;
    }
    let merged = Library::new(profile, history, deleted);
    (changed, merged)
}

/// True when the merged library says something the remote one didn't, so it's worth sending back.
pub fn differs(remote: Option<&Library>, merged: &Library) -> bool {
    remote.map_or(true, |r| serde_json::to_value(r).ok() != serde_json::to_value(merged).ok())
}

/// Two-way sync with `folder/yap-library.json`.
pub fn sync(folder: &Path, profile: &mut Profile, history: &mut Vec<Dictation>, deleted: &mut Vec<String>) -> Result<bool, String> {
    if !folder.is_dir() {
        return Err(format!("The library folder {} isn't there anymore.", folder.display()));
    }
    let path = folder.join(FILE);
    let remote = if path.exists() { Some(read(&path)?) } else { None };
    let (changed, merged) = merge(remote.as_ref(), profile, history, deleted);
    if differs(remote.as_ref(), &merged) {
        write(&path, &merged)?;
    }
    Ok(changed)
}
