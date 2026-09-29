//! Notes: a meeting recorder in the spirit of Granola. Press record and it keeps listening,
//! transcribing as it goes with your local Whisper model, while you type your own rough notes
//! alongside. Stop, and it lays out what happened: your notes, then the action items, decisions
//! and open questions it heard, then the transcript. Nothing leaves the Mac.
//!
//! Two ways to record. In person, the mic hears the room. On a call, the mic is you and the
//! Mac's own sound (through ScreenCaptureKit: audio only, never the screen) is everyone else,
//! so the transcript can say who said what.

use crate::polish;
use crate::store::Profile;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Segment {
    /// Seconds from the start of the recording.
    pub at: f32,
    /// "you" or "them" on a call. "room" in person, where the mic hears everyone.
    pub who: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub created_at: String,
    /// "person" or "call"
    pub mode: String,
    pub duration_secs: f32,
    pub segments: Vec<Segment>,
    /// What you typed while it listened: the skeleton of the notes, as in Granola.
    pub my_notes: String,
    /// The enhanced notes as Markdown, for copying: yours, with the transcript's details added.
    pub summary: String,
    /// The same enhanced notes, block by block, each marked yours or from the transcript so the
    /// two can be told apart on screen, the way Granola greys out what it added.
    pub enhanced: Vec<Block>,
    /// Something worth knowing, like why the other side of a call wasn't heard.
    pub warning: String,
}

/// One line of the enhanced notes.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Block {
    /// "heading", "bullet" or "text"
    pub kind: String,
    pub text: String,
    /// How far a bullet is indented.
    pub depth: u8,
    /// Added from the transcript, rather than something you typed.
    pub from_transcript: bool,
    /// Where in the recording a transcript line came from, in seconds.
    pub at: Option<f32>,
    /// Who said a transcript line: "you", "them" or "room".
    pub who: String,
}

// ---------- laying out the notes ----------

/// Sentences that commit someone to something.
static ACTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)\b(?:
            i'?ll | i \s+ will | i'?m \s+ (?: going \s+ to | gonna ) | we'?ll | we \s+ will
          | (?: i | we | you ) \s+ (?: need | have ) \s+ to | we \s+ should | let'?s
          | can \s+ you | could \s+ you | would \s+ you \s+ mind | make \s+ sure | follow \s+ up
          | action \s+ item | to-?do | remind \s+ me
          | by \s+ (?: monday | tuesday | wednesday | thursday | friday | tomorrow | tonight | eod
                    | end \s+ of \s+ (?: the \s+ )? (?: day | week ) | next \s+ week )
        )\b",
    )
    .unwrap()
});

/// Sentences that settle something. Checked before actions: "let's go with B" is a decision.
static DECISION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)\b(?:
            we \s+ decided | decided \s+ to | we'?re \s+ going \s+ (?: with | to \s+ go \s+ with )
          | let'?s \s+ go \s+ with | we'?ll \s+ go \s+ with | we \s+ agreed | agreed \s+ (?: to | on | that )
          | the \s+ plan \s+ is | going \s+ forward | settled \s+ on
        )\b",
    )
    .unwrap()
});

/// Meeting chatter that sounds like a commitment and isn't: "let's get started".
static NOT_ACTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)\blet'?s \s+ (?:
            get \s+ (?: started | going ) | start | begin | kick \s+ (?: it \s+ )? off | dive \s+ in
          | jump \s+ in | wrap \s+ (?: it \s+ )? up | move \s+ on | see | recap
        )\b",
    )
    .unwrap()
});

/// "Great, on the beta, we decided…": how it was said, not part of the point.
static LEAD_IN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:(?:great|okay|ok|yeah|yes|so|sure|right|cool|alright|well|um|uh|and|but|also)\b[,.]?\s+)+").unwrap()
});

/// Where a spoken point ends when Whisper didn't punctuate it.
static TRAILS_OFF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s+\b(?:then|anyway|anything\s+else|okay|alright|so\s+yeah|thanks|thank\s+you|right)\b").unwrap()
});

/// Longer than this and a "sentence" is really Whisper running several together.
const RUN_ON: usize = 20;

/// The point itself, from a sentence that matched `re`: lead-ins dropped, and an unpunctuated
/// run-on cut down to the part around what matched, instead of the whole paragraph.
fn point(sentence: &str, re: &Regex) -> String {
    let mut s = sentence.trim().trim_end_matches('.').to_string();
    if s.split_whitespace().count() > RUN_ON {
        if let Some(m) = re.find(&s) {
            let from = s[m.start()..].to_string();
            let end = TRAILS_OFF.find(&from[m.end() - m.start()..]).map_or(from.len(), |t| t.start() + m.end() - m.start());
            s = from[..end].split_whitespace().take(14).collect::<Vec<_>>().join(" ");
        }
    }
    let s = LEAD_IN.replace(&s, "").trim().to_string();
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Words that recur in any meeting without saying what it was about.
const NOT_TOPICS: &[&str] = &[
    "three", "four", "five", "seven", "eight", "nine", "twenty", "thirty", "hundred", "thousand",
    "first", "second", "third", "great", "cool", "totally", "exactly", "honestly", "basically", "happens",
];

/// What Whisper writes over silence. Only dropped when the audio behind it was quiet too.
const HALLUCINATIONS: &[&str] = &["thank you", "thanks for watching", "thank you for watching", "you", "bye", "okay"];

/// True when a chunk's text is Whisper filling silence rather than someone speaking.
pub fn is_noise(text: &str, loudness: f32) -> bool {
    let t: String = text.to_lowercase().chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect();
    let t = t.trim();
    t.is_empty() || (loudness < 0.02 && HALLUCINATIONS.contains(&t))
}

fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn duration(secs: f32) -> String {
    let m = (secs / 60.0).round() as u32;
    match m {
        0 => format!("{} sec", secs.round() as u32),
        m if m < 60 => format!("{m} min"),
        m => format!("{} hr {} min", m / 60, m % 60),
    }
}

fn words(s: &str) -> HashSet<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 2).map(str::to_lowercase).collect()
}

/// On a call with speakers rather than headphones, your mic also hears the other side, so the
/// same words turn up twice. A "you" line that's mostly a nearby "them" line is that echo.
fn without_echo(segments: &[Segment]) -> Vec<Segment> {
    let them: Vec<(f32, HashSet<String>)> =
        segments.iter().filter(|s| s.who == "them").map(|s| (s.at, words(&s.text))).collect();
    segments
        .iter()
        .filter(|s| {
            if s.who != "you" {
                return true;
            }
            let mine = words(&s.text);
            if mine.len() < 4 {
                return true;
            }
            !them.iter().any(|(at, theirs)| {
                (at - s.at).abs() < 30.0 && mine.intersection(theirs).count() as f32 / mine.len() as f32 >= 0.6
            })
        })
        .cloned()
        .collect()
}

/// A sentence from the transcript, cleaned up, with what it's about.
struct Said {
    text: String,
    at: f32,
    who: String,
    words: HashSet<String>,
}

fn about(text: &str) -> HashSet<String> {
    polish::subject_words(text).into_iter().filter(|w| !NOT_TOPICS.contains(&w.as_str())).collect()
}

/// Every sentence worth keeping, in the order it was said, each once.
fn said(note: &Note, profile: &Profile) -> Vec<Said> {
    let mut segments = without_echo(&note.segments);
    segments.sort_by(|a, b| a.at.total_cmp(&b.at));
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for seg in segments {
        // The same cleanup as dictation: fillers, stutters, capitals, your dictionary.
        let clean = polish::local_rules(profile, &seg.text).text;
        for sentence in polish::sentences_with_ends(&clean) {
            let sentence = sentence.trim();
            if sentence.split_whitespace().count() >= 4 && seen.insert(sentence.to_lowercase()) {
                out.push(Said { text: sentence.to_string(), at: seg.at, who: seg.who.clone(), words: about(sentence) });
            }
        }
    }
    out
}

/// How much a word says about which sentence is meant: rare words a lot, common ones a little.
fn rarity(said: &[Said]) -> HashMap<String, f32> {
    let mut count: HashMap<String, f32> = HashMap::new();
    for s in said {
        for w in &s.words {
            *count.entry(w.clone()).or_default() += 1.0;
        }
    }
    let n = said.len() as f32;
    count.into_iter().map(|(w, df)| (w, (1.0 + n / df).ln())).collect()
}

fn overlap(a: &HashSet<String>, b: &HashSet<String>, rarity: &HashMap<String, f32>) -> f32 {
    a.intersection(b).map(|w| rarity.get(w).copied().unwrap_or(0.0)).sum()
}

/// Your notes, line by line: headings, bullets (with how deep) and plain lines.
fn yours(my_notes: &str) -> Vec<(String, String, u8)> {
    let mut out = Vec::new();
    for line in my_notes.lines() {
        let indent = line.chars().take_while(|c| *c == ' ').count() + 4 * line.chars().take_while(|c| *c == '\t').count();
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let depth = (indent / 2).min(4) as u8;
        if let Some(h) = t.strip_prefix("### ").or(t.strip_prefix("## ")).or(t.strip_prefix("# ")) {
            out.push(("heading".into(), h.trim().to_string(), 0));
        } else if let Some(b) = ["- ", "* ", "• "].iter().find_map(|p| t.strip_prefix(p)) {
            out.push(("bullet".into(), b.trim().to_string(), depth));
        } else if t.ends_with(':') && t.split_whitespace().count() <= 6 {
            out.push(("heading".into(), t.trim_end_matches(':').trim().to_string(), 0));
        } else {
            out.push(("text".into(), t.to_string(), depth));
        }
    }
    out
}

/// A transcript sentence made readable as a note: lead-ins dropped, and an unpunctuated run-on
/// clipped to the stretch around what it's about, marked with "…" where it was cut.
fn tidy(text: &str, around: &HashSet<String>) -> String {
    // A run-on that commits to or settles something: the point is that part, as for action items.
    if text.split_whitespace().count() > RUN_ON {
        for re in [&*DECISION, &*ACTION] {
            if re.is_match(text) {
                return point(text, re);
            }
        }
    }
    let text = LEAD_IN.replace(text.trim().trim_end_matches('.'), "").trim().to_string();
    let words: Vec<&str> = text.split_whitespace().collect();
    let clipped = if words.len() > RUN_ON {
        let hit = words.iter().position(|w| {
            let w: String = w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
            around.contains(&w) || around.contains(w.trim_end_matches('s'))
        });
        let from = hit.map_or(0, |i| i.saturating_sub(3));
        let mut to = (from + 14).min(words.len());
        // End where the speaker trailed off ("then", "anyway", "thanks"), if that's sooner.
        let window = words[from..to].join(" ");
        if let Some(m) = TRAILS_OFF.find(&window) {
            to = from + window[..m.start()].split_whitespace().count();
        }
        let mut piece = words[from..to].join(" ");
        if from > 0 {
            piece = format!("…{piece}");
        }
        if to < words.len() {
            piece += "…";
        }
        piece
    } else {
        text
    };
    let mut c = clipped.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn from_transcript(kind: &str, text: String, depth: u8, s: Option<&Said>) -> Block {
    Block {
        kind: kind.into(),
        text,
        depth,
        from_transcript: true,
        at: s.map(|s| s.at),
        who: s.map(|s| s.who.clone()).unwrap_or_default(),
    }
}

/// When you didn't type anything, the meeting in topics: sentences grouped by what they share,
/// and the ones that say the most about each group.
fn key_points(said: &[Said], rarity: &HashMap<String, f32>, used: &mut [bool]) -> Vec<Block> {
    let mut groups: Vec<(HashSet<String>, Vec<usize>)> = Vec::new();
    for (i, s) in said.iter().enumerate() {
        let best = groups
            .iter()
            .enumerate()
            .map(|(g, (bag, _))| (g, overlap(&s.words, bag, rarity)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, score)| *score >= 1.0);
        match best {
            Some((g, _)) => {
                groups[g].0.extend(s.words.iter().cloned());
                groups[g].1.push(i);
            }
            None => groups.push((s.words.clone(), vec![i])),
        }
    }
    let mut out = Vec::new();
    for (bag, members) in groups.iter().filter(|(_, m)| m.len() >= 2).take(6) {
        // Named for the word that runs through the group, weighted so a rare one beats a common one.
        let mut weight: HashMap<&String, f32> = HashMap::new();
        for i in members {
            for w in &said[*i].words {
                *weight.entry(w).or_default() += rarity.get(w).copied().unwrap_or(0.0);
            }
        }
        let name = weight.into_iter().max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(a.0))).map(|(w, _)| w.clone());
        let Some(name) = name else { continue };
        let mut c = name.chars();
        let heading = c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
        out.push(from_transcript("heading", heading, 0, None));
        let mut best: Vec<usize> = members.clone();
        // What a sentence says about the group, less for an unpunctuated run-on, which is Whisper
        // gluing several thoughts together rather than one point.
        let fit = |i: usize| {
            let run_on = said[i].text.split_whitespace().count() > RUN_ON;
            overlap(&said[i].words, bag, rarity) * if run_on { 0.4 } else { 1.0 }
        };
        best.sort_by(|a, b| fit(*b).total_cmp(&fit(*a)));
        best.truncate(3);
        best.sort();
        for i in best {
            used[i] = true;
            out.push(from_transcript("bullet", tidy(&said[i].text, bag), 0, Some(&said[i])));
        }
    }
    // Too short or too scattered for topics: the lines that carry the most.
    if out.is_empty() {
        let mut best: Vec<usize> = (0..said.len()).collect();
        let weight = |i: usize| said[i].words.iter().map(|w| rarity.get(w).copied().unwrap_or(0.0)).sum::<f32>();
        best.sort_by(|a, b| weight(*b).total_cmp(&weight(*a)));
        best.truncate(5);
        best.sort();
        if !best.is_empty() {
            out.push(from_transcript("heading", "Key points".into(), 0, None));
        }
        for i in best {
            used[i] = true;
            out.push(from_transcript("bullet", tidy(&said[i].text, &said[i].words), 0, Some(&said[i])));
        }
    }
    out
}

/// Enhanced notes, the way Granola does it: your notes stay as you wrote them, each followed by
/// what was said about it, and then what was committed to, settled and left open. With nothing
/// typed, the meeting is grouped into topics instead.
pub fn enhance(note: &Note, profile: &Profile) -> Vec<Block> {
    let said = said(note, profile);
    let rarity = rarity(&said);
    let mut used = vec![false; said.len()];
    let mut out = Vec::new();

    let mine = yours(&note.my_notes);
    for (kind, text, depth) in &mine {
        out.push(Block { kind: kind.clone(), text: text.clone(), depth: *depth, ..Default::default() });
        if kind == "heading" {
            continue;
        }
        // The two sentences most about this line, not already used under another one.
        let words = about(text);
        let mut matches: Vec<(f32, usize)> = said
            .iter()
            .enumerate()
            .filter(|(i, _)| !used[*i])
            .map(|(i, s)| (overlap(&words, &s.words, &rarity), i))
            .filter(|(score, _)| *score > 0.0)
            .collect();
        matches.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        matches.truncate(2);
        matches.sort_by_key(|(_, i)| *i);
        for (_, i) in matches {
            used[i] = true;
            out.push(from_transcript("bullet", tidy(&said[i].text, &words), depth + 1, Some(&said[i])));
        }
    }
    if mine.is_empty() {
        out.extend(key_points(&said, &rarity, &mut used));
    }

    let (mut actions, mut decisions, mut questions) = (Vec::new(), Vec::new(), Vec::new());
    for s in &said {
        let is_action = ACTION.is_match(&s.text) && !(NOT_ACTION.is_match(&s.text) && ACTION.find_iter(&s.text).count() == 1);
        if DECISION.is_match(&s.text) {
            decisions.push(from_transcript("bullet", point(&s.text, &DECISION), 0, Some(s)));
        } else if is_action {
            actions.push(from_transcript("bullet", point(&s.text, &ACTION), 0, Some(s)));
        } else if s.text.ends_with('?') {
            questions.push(from_transcript("bullet", point(&s.text, &ACTION), 0, Some(s)));
        }
    }
    for (heading, items) in [("Action items", actions), ("Decisions", decisions), ("Open questions", questions)] {
        if !items.is_empty() {
            out.push(from_transcript("heading", heading.into(), 0, None));
            out.extend(items);
        }
    }
    out
}

/// The enhanced notes as Markdown, so they paste into Notion, Slack or an email as they look.
pub fn markdown(note: &Note, blocks: &[Block]) -> String {
    let date = chrono::DateTime::parse_from_rfc3339(&note.created_at)
        .map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %-I:%M %p").to_string())
        .unwrap_or_default();
    let two_sides = note.segments.iter().any(|s| s.who == "them");
    let mut out = format!("# {}\n{} · {}\n", note.title.trim(), date, duration(note.duration_secs));
    let mut after_heading = false;
    for b in blocks {
        match b.kind.as_str() {
            "heading" => {
                out += &format!("\n## {}\n", b.text);
                after_heading = true;
            }
            kind => {
                if !after_heading && out.ends_with('\n') && !out.ends_with("\n\n") && kind == "text" {
                    out += "\n";
                }
                let who = if two_sides && b.from_transcript && b.who == "them" { " (them)" } else { "" };
                let indent = "  ".repeat(b.depth as usize);
                if kind == "bullet" {
                    out += &format!("{indent}- {}{who}\n", b.text);
                } else {
                    out += &format!("{indent}{}{who}\n", b.text);
                }
                after_heading = false;
            }
        }
    }
    if blocks.is_empty() {
        out += "\nNothing was said yet.\n";
    }
    out
}

/// Rebuilds a note's enhanced notes, both the blocks for the screen and the Markdown for copying.
pub fn lay_out(note: &mut Note, profile: &Profile) {
    note.enhanced = enhance(note, profile);
    note.summary = markdown(note, &note.enhanced);
}

/// The transcript as text, for copying on its own.
pub fn transcript(note: &Note) -> String {
    let two_sides = note.segments.iter().any(|s| s.who == "them");
    let mut segments = without_echo(&note.segments);
    segments.sort_by(|a, b| a.at.total_cmp(&b.at));
    segments
        .iter()
        .map(|s| {
            let who = match (two_sides, s.who.as_str()) {
                (true, "you") => "You · ",
                (true, "them") => "Them · ",
                _ => "",
            };
            format!("{who}{}  {}", clock(s.at), s.text.trim())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A default title: the first line of your notes, or when and how it was recorded.
pub fn default_title(note: &Note) -> String {
    let first = note.my_notes.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let first = first.trim_start_matches(['#', '-', '*', ' ']).trim();
    if !first.is_empty() {
        return first.chars().take(60).collect();
    }
    let date = chrono::DateTime::parse_from_rfc3339(&note.created_at)
        .map(|d| d.with_timezone(&chrono::Local).format("%b %-d").to_string())
        .unwrap_or_default();
    format!("{} · {date}", if note.mode == "call" { "Call" } else { "Meeting" })
}

// ---------- recording ----------

#[cfg(desktop)]
pub use capture::{start, Chunk, Session};
#[cfg(all(desktop, test))]
pub use capture::chunks_of;

#[cfg(desktop)]
mod capture {
    use crate::audio;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread::JoinHandle;
    use std::time::Duration;

    const RATE: usize = audio::RATE as usize;
    /// Longest piece handed to Whisper at once.
    const MAX_CHUNK: usize = 20 * RATE;
    /// A pause this long after at least `EARLY` of speech ends a piece early, so the transcript
    /// keeps up with you instead of lagging twenty seconds behind.
    const PAUSE: usize = RATE * 8 / 10;
    const EARLY: usize = 6 * RATE;
    /// Below this, a piece is room tone, not speech, and isn't worth transcribing.
    const QUIET: f32 = 0.004;

    /// A piece of audio ready for Whisper.
    pub struct Chunk {
        pub who: &'static str,
        /// Seconds from the start of the recording.
        pub at: f32,
        pub samples: Vec<f32>,
        pub loudness: f32,
    }

    /// One source (the mic, or the Mac's sound) buffered at 16 kHz and cut at quiet moments.
    struct Track {
        who: &'static str,
        rate: u32,
        incoming: Arc<Mutex<Vec<f32>>>,
        pending: Vec<f32>,
        /// Samples already handed off, so chunks know where they sit in the recording.
        done: usize,
        level: f32,
    }

    impl Track {
        fn new(who: &'static str, rate: u32, incoming: Arc<Mutex<Vec<f32>>>) -> Self {
            Self { who, rate, incoming, pending: Vec::new(), done: 0, level: 0.0 }
        }

        fn pull(&mut self) {
            let raw = std::mem::take(&mut *self.incoming.lock().unwrap());
            let fresh = audio::resample(&raw, self.rate);
            self.level = audio::rms(&fresh);
            self.pending.extend(fresh);
        }

        fn quiet_at(&self, from: usize, to: usize) -> usize {
            const WIN: usize = RATE / 10;
            (from..to.saturating_sub(WIN))
                .step_by(WIN / 2)
                .min_by(|a, b| audio::rms(&self.pending[*a..*a + WIN]).total_cmp(&audio::rms(&self.pending[*b..*b + WIN])))
                .map_or(to, |i| i + WIN / 2)
        }

        /// The next piece to transcribe, if one is ready. `all` at the end takes everything.
        fn next(&mut self, all: bool) -> Option<Chunk> {
            let len = self.pending.len();
            let cut = if all {
                len
            } else if len >= MAX_CHUNK {
                self.quiet_at(MAX_CHUNK * 3 / 5, MAX_CHUNK)
            } else if len >= EARLY && audio::rms(&self.pending[len - PAUSE..]) < QUIET {
                len
            } else {
                return None;
            };
            if cut == 0 {
                return None;
            }
            let samples: Vec<f32> = self.pending.drain(..cut).collect();
            let at = self.done as f32 / RATE as f32;
            self.done += samples.len();
            let loudness = audio::rms(&samples);
            (loudness >= QUIET && samples.len() > RATE / 2).then_some(Chunk { who: self.who, at, samples, loudness })
        }
    }

    /// Runs audio through the same cutting as a live recording, a quarter-second at a time, for tests.
    #[cfg(test)]
    pub fn chunks_of(who: &'static str, samples: &[f32]) -> Vec<Chunk> {
        let incoming = Arc::new(Mutex::new(Vec::new()));
        let mut track = Track::new(who, audio::RATE, incoming.clone());
        let mut out = Vec::new();
        for piece in samples.chunks(RATE / 4) {
            incoming.lock().unwrap().extend_from_slice(piece);
            track.pull();
            while let Some(c) = track.next(false) {
                out.push(c);
            }
        }
        while let Some(c) = track.next(true) {
            out.push(c);
        }
        out
    }

    /// A recording in progress: a thread capturing and cutting audio, and a thread feeding the
    /// pieces to Whisper in order.
    pub struct Session {
        stop: Arc<AtomicBool>,
        capture: Option<JoinHandle<f32>>,
        worker: Option<JoinHandle<()>>,
    }

    impl Session {
        /// Stops listening, transcribes what's left, and returns how long it ran, in seconds.
        pub fn finish(mut self) -> f32 {
            self.stop.store(true, Ordering::SeqCst);
            let secs = self.capture.take().and_then(|h| h.join().ok()).unwrap_or(0.0);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
            secs
        }
    }

    /// Starts recording. `call` adds the Mac's own sound as "them". Returns the session and, if
    /// the other side of a call couldn't be heard, why.
    pub fn start(
        call: bool,
        transcribe: impl Fn(Chunk) + Send + 'static,
        levels: impl Fn(f32, f32) + Send + 'static,
    ) -> Result<(Session, Option<String>), String> {
        let stop = Arc::new(AtomicBool::new(false));
        let (chunks, inbox) = mpsc::channel::<Chunk>();
        let worker = std::thread::spawn(move || {
            for chunk in inbox {
                transcribe(chunk);
            }
        });

        // The streams aren't Send, so they're opened, kept and dropped on the capture thread.
        let (opened, ready) = mpsc::channel::<Result<Option<String>, String>>();
        let stopping = stop.clone();
        let capture = std::thread::spawn(move || {
            let mic_in = Arc::new(Mutex::new(Vec::new()));
            let sink = mic_in.clone();
            let (mic, mic_rate) = match audio::open_streaming(move |s| sink.lock().unwrap().extend_from_slice(s)) {
                Ok(m) => m,
                Err(e) => {
                    let _ = opened.send(Err(e));
                    return 0.0;
                }
            };
            let mut tracks = vec![Track::new(if call { "you" } else { "room" }, mic_rate, mic_in)];
            let mut warning = None;
            #[cfg(target_os = "macos")]
            let _system = if call {
                let sys_in = Arc::new(Mutex::new(Vec::new()));
                match super::system::start(sys_in.clone()) {
                    Ok(capture) => {
                        tracks.push(Track::new("them", super::system::RATE, sys_in));
                        Some(capture)
                    }
                    Err(e) => {
                        warning = Some(e);
                        None
                    }
                }
            } else {
                None
            };
            #[cfg(not(target_os = "macos"))]
            if call {
                warning = Some("Hearing the other side of a call only works on a Mac. Recording your mic.".into());
            }
            let _ = opened.send(Ok(warning));

            while !stopping.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(250));
                for track in &mut tracks {
                    track.pull();
                    while let Some(chunk) = track.next(false) {
                        let _ = chunks.send(chunk);
                    }
                }
                let them = tracks.get(1).map_or(0.0, |t| t.level);
                levels(tracks[0].level, them);
            }
            drop(mic);
            let mut secs = 0.0_f32;
            for track in &mut tracks {
                track.pull();
                while let Some(chunk) = track.next(true) {
                    let _ = chunks.send(chunk);
                }
                secs = secs.max(track.done as f32 / RATE as f32);
            }
            secs
        });

        match ready.recv().map_err(|e| e.to_string())? {
            Ok(warning) => Ok((Session { stop, capture: Some(capture), worker: Some(worker) }, warning)),
            Err(e) => {
                stop.store(true, Ordering::SeqCst);
                let _ = capture.join();
                Err(e)
            }
        }
    }
}

/// The Mac's own sound, for the other side of a call. ScreenCaptureKit is asked for audio and a
/// 2×2 picture it throws away, because it won't run without a picture.
#[cfg(target_os = "macos")]
mod system {
    use screencapturekit::prelude::*;
    use std::sync::{Arc, Mutex};

    pub const RATE: u32 = 48_000;

    pub struct Capture {
        stream: SCStream,
    }

    impl Drop for Capture {
        fn drop(&mut self) {
            let _ = self.stream.stop_capture();
        }
    }

    struct Sound {
        incoming: Arc<Mutex<Vec<f32>>>,
    }

    impl SCStreamOutputTrait for Sound {
        fn did_output_sample_buffer(&self, sample: CMSampleBuffer, kind: SCStreamOutputType) {
            if !matches!(kind, SCStreamOutputType::Audio) {
                return;
            }
            if let Some(list) = sample.audio_buffer_list() {
                // One channel was asked for, so the first buffer is all of it.
                if let Some(buffer) = (&list).into_iter().next() {
                    let floats = buffer.data().chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                    self.incoming.lock().unwrap().extend(floats);
                }
            }
        }
    }

    pub fn start(incoming: Arc<Mutex<Vec<f32>>>) -> Result<Capture, String> {
        let denied = || {
            "Couldn't hear the other side of the call. Allow Yap under System Settings → Privacy & Security → \
             Screen & System Audio Recording, then record again. This one is recording your mic only."
                .to_string()
        };
        let content = SCShareableContent::get().map_err(|_| denied())?;
        let display = content.displays().into_iter().next().ok_or_else(denied)?;
        let filter = SCContentFilter::create().with_display(&display).with_excluding_windows(&[]).build();
        let config = SCStreamConfiguration::new()
            .with_width(2)
            .with_height(2)
            .with_captures_audio(true)
            .with_sample_rate(RATE as i32)
            .with_channel_count(1)
            // Yap's own sounds aren't part of the conversation.
            .with_excludes_current_process_audio(true);
        let mut stream = SCStream::new(&filter, &config);
        stream.add_output_handler(Sound { incoming }, SCStreamOutputType::Audio);
        stream.start_capture().map_err(|_| denied())?;
        Ok(Capture { stream })
    }
}
