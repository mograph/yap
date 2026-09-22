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
    /// The laid-out notes, rebuilt from your notes and the transcript whenever either changes.
    pub summary: String,
    /// Something worth knowing, like why the other side of a call wasn't heard.
    pub warning: String,
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

/// Lays out a note: your notes first, then what the transcript committed to, settled and left
/// open, then what came up, then the transcript itself. Plain Markdown, so it pastes anywhere.
pub fn summarize(note: &Note, profile: &Profile) -> String {
    let mut segments = without_echo(&note.segments);
    segments.sort_by(|a, b| a.at.total_cmp(&b.at));
    // The same cleanup as dictation: fillers, stutters, capitals, your dictionary.
    let cleaned: Vec<Segment> = segments
        .iter()
        .map(|s| Segment { text: polish::local_rules(profile, &s.text).text, ..s.clone() })
        .filter(|s| !s.text.trim().is_empty())
        .collect();

    let (mut actions, mut decisions, mut questions) = (Vec::new(), Vec::new(), Vec::new());
    let mut seen = HashSet::new();
    let mut topics: HashMap<String, usize> = HashMap::new();
    for seg in &cleaned {
        for sentence in polish::sentences_with_ends(&seg.text) {
            let sentence = sentence.trim();
            if sentence.split_whitespace().count() < 4 || !seen.insert(sentence.to_lowercase()) {
                continue;
            }
            let tagged = |s: String| if seg.who == "them" { format!("{s} (them)") } else { s };
            let is_action = ACTION.is_match(sentence) && !(NOT_ACTION.is_match(sentence) && ACTION.find_iter(sentence).count() == 1);
            if DECISION.is_match(sentence) {
                decisions.push(tagged(point(sentence, &DECISION)));
            } else if is_action {
                actions.push(tagged(point(sentence, &ACTION)));
            } else if sentence.ends_with('?') {
                questions.push(tagged(point(sentence, &ACTION)));
            }
        }
        for w in polish::subject_words(&seg.text) {
            if !NOT_TOPICS.contains(&w.as_str()) {
                *topics.entry(w).or_default() += 1;
            }
        }
    }

    let mut out = String::new();
    let mode = if note.mode == "call" { "On a call" } else { "In person" };
    let date = chrono::DateTime::parse_from_rfc3339(&note.created_at)
        .map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %-I:%M %p").to_string())
        .unwrap_or_default();
    out += &format!("# {}\n{} · {} · {}\n", note.title.trim(), date, duration(note.duration_secs), mode);

    let mine = note.my_notes.trim();
    if !mine.is_empty() {
        out += &format!("\n## Your notes\n{mine}\n");
    }
    for (heading, items) in [("Action items", &actions), ("Decisions", &decisions), ("Open questions", &questions)] {
        if !items.is_empty() {
            out += &format!("\n## {heading}\n");
            for item in items.iter() {
                out += &format!("- {}\n", item.trim_end_matches('.'));
            }
        }
    }
    let mut common: Vec<(String, usize)> = topics.into_iter().filter(|(_, n)| *n >= 2).collect();
    common.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    if !common.is_empty() {
        let names: Vec<String> = common
            .iter()
            .take(6)
            .map(|(w, _)| {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            })
            .collect();
        out += &format!("\n## Came up\n{}\n", names.join(", "));
    }
    if !cleaned.is_empty() {
        out += "\n## Transcript\n";
        for seg in &cleaned {
            let who = match seg.who.as_str() {
                "you" => "You · ",
                "them" => "Them · ",
                _ => "",
            };
            out += &format!("{who}{}  {}\n", clock(seg.at), seg.text.trim());
        }
    } else {
        out += "\nNothing was transcribed.\n";
    }
    out
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
