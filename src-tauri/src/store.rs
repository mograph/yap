//! Settings, voice profile and dictation history, persisted as JSON in the app data dir.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

/// Every kind of change the cleanup step can make. The UI groups edits and charts by these.
pub const KINDS: [&str; 9] = [
    "filler",
    "correction",
    "punctuation",
    "grammar",
    "slang",
    "rephrase",
    "swearing",
    "formatting",
    "dictionary",
];

/// What the user allows for one kind of change.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Rule {
    /// Make the change.
    Do,
    /// Don't make it, but show it as a suggestion.
    Suggest,
    /// Don't touch it, don't mention it.
    Leave,
}

impl Rule {
    pub fn as_str(self) -> &'static str {
        match self {
            Rule::Do => "do",
            Rule::Suggest => "suggest",
            Rule::Leave => "leave",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// What you hold to talk: "option", "right-option", "fn" (macOS), or "shortcut" (the combo below)
    pub trigger: String,
    pub shortcut: String,
    /// "local" (a model on this machine) or "cloud" (any OpenAI-compatible transcription API)
    pub stt_engine: String,
    /// Catalog id from `stt::CATALOG`
    #[serde(alias = "whisperModel")]
    pub local_model: String,
    /// "auto" or an ISO code like "en"
    pub language: String,
    pub cloud_stt_url: String,
    pub cloud_stt_model: String,
    pub cloud_stt_key: String,
    /// "claude" or "local" (rules only, no API)
    pub brain: String,
    pub claude_model: String,
    pub anthropic_key: String,
    pub auto_paste: bool,
    pub restore_clipboard: bool,
    pub sounds: bool,
    pub onboarded: bool,
    /// When there's a tidier arrangement on offer: "ask", "auto" (always take it) or "never"
    pub lists: String,
    /// A synced folder holding yap-library.json, or "" for no syncing
    pub library_folder: String,
    /// Keep an encrypted copy of the library in Firestore too
    pub cloud_sync: bool,
    pub firebase_project_id: String,
    /// Not a secret: Firebase web API keys identify the project, security rules do the guarding.
    pub firebase_api_key: String,
    /// From an OAuth "Desktop app" client. Only needed to sign in with Google.
    pub google_client_id: String,
    pub google_client_secret: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            trigger: if cfg!(target_os = "macos") { "option" } else { "shortcut" }.into(),
            shortcut: if cfg!(target_os = "macos") { "Alt+Space" } else { "Ctrl+Shift+Space" }.into(),
            stt_engine: "local".into(),
            local_model: "large-v3-turbo-q5_0".into(),
            language: "auto".into(),
            cloud_stt_url: "https://api.openai.com/v1/audio/transcriptions".into(),
            cloud_stt_model: "whisper-large-v3-turbo".into(),
            cloud_stt_key: String::new(),
            brain: "claude".into(),
            claude_model: "claude-opus-5".into(),
            anthropic_key: String::new(),
            auto_paste: true,
            restore_clipboard: true,
            sounds: true,
            onboarded: false,
            lists: "ask".into(),
            library_folder: String::new(),
            cloud_sync: false,
            firebase_project_id: String::new(),
            firebase_api_key: String::new(),
            google_client_id: String::new(),
            google_client_secret: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DictEntry {
    pub say: String,
    pub write: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    pub name: String,
    /// 0 = exactly how I talk, 100 = polished
    pub tone: u8,
    /// Write it like a Slack message: no structure imposed, never more formal than chatty.
    pub casual: bool,
    pub rules: BTreeMap<String, Rule>,
    /// Words and phrases that must never be "fixed".
    pub my_words: Vec<String>,
    pub dictionary: Vec<DictEntry>,
    /// Pasted examples of how the user actually writes.
    pub samples: String,
    /// Free-form notes, e.g. "I never use capital letters in Slack".
    pub notes: String,
    /// Milliseconds since 1970 of the last real edit; the newest wins when libraries sync.
    pub updated_at: i64,
}

impl Default for Profile {
    fn default() -> Self {
        let rules = [
            ("filler", Rule::Do),
            ("correction", Rule::Do),
            ("punctuation", Rule::Do),
            ("grammar", Rule::Suggest),
            ("slang", Rule::Leave),
            ("rephrase", Rule::Suggest),
            ("swearing", Rule::Leave),
            ("formatting", Rule::Do),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        Self {
            name: String::new(),
            tone: 25,
            casual: false,
            rules,
            my_words: vec!["gonna".into(), "wanna".into(), "kinda".into()],
            dictionary: vec![
                DictEntry { say: "tinker studio".into(), write: "Tinker Studio".into() },
                DictEntry { say: "claude code".into(), write: "Claude Code".into() },
            ],
            samples: String::new(),
            notes: String::new(),
            updated_at: 0,
        }
    }
}

impl Profile {
    pub fn rule(&self, kind: &str) -> Rule {
        if kind == "dictionary" {
            return Rule::Do;
        }
        self.rules.get(kind).copied().unwrap_or(Rule::Do)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Edit {
    pub original: String,
    pub replacement: String,
    pub kind: String,
    /// true = made the change, false = only suggested
    pub applied: bool,
    pub why: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Dictation {
    pub id: String,
    pub created_at: String,
    pub raw: String,
    pub text: String,
    pub edits: Vec<Edit>,
    /// Which brain produced `text`, e.g. "claude-opus-5" or "local rules"
    pub engine: String,
    /// Anything the user should know, e.g. why Claude was skipped
    pub note: String,
    pub words: usize,
    /// Share of the speaker's own words (fillers excluded) that survived cleanup
    pub kept_pct: f32,
    pub audio_secs: f32,
    pub stt_ms: u64,
    pub polish_ms: u64,
    /// The tidier arrangement, when there was one and it wasn't used
    #[serde(skip_serializing_if = "String::is_empty")]
    pub list: String,
}

pub fn load<T: DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(text) = fs::read_to_string(path) else {
        return T::default();
    };
    match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            // Keep the unreadable file around instead of silently overwriting it later.
            eprintln!("yap: couldn't read {}: {e}", path.display());
            let _ = fs::rename(path, path.with_extension("json.corrupt"));
            T::default()
        }
    }
}

pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

const FILLERS: [&str; 9] = ["um", "umm", "uh", "uhh", "uhm", "erm", "er", "ah", "hmm"];

fn words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

pub fn word_count(s: &str) -> usize {
    words(s).len()
}

/// Percentage of the raw transcript's words (ignoring um/uh) that appear, in order, in the final text.
pub fn kept_pct(raw: &str, text: &str) -> f32 {
    let a: Vec<String> = words(raw)
        .into_iter()
        .filter(|w| !FILLERS.contains(&w.as_str()))
        .collect();
    let b = words(text);
    if a.is_empty() {
        return 100.0;
    }
    let mut prev = vec![0usize; b.len() + 1];
    for x in &a {
        let mut cur = vec![0usize; b.len() + 1];
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = if x == y { prev[j] + 1 } else { cur[j].max(prev[j + 1]) };
        }
        prev = cur;
    }
    prev[b.len()] as f32 / a.len() as f32 * 100.0
}
