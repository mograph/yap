//! Turns a raw transcript from a Notes recording into a structured note: a title, a
//! summary, a few chapters, any decisions made, and any action items volunteered or
//! assigned. Modeled on open-granola's EnhancedNote shape and enhance-prompt strategy
//! (https://github.com/anshuman-pandey/open-granola, Apache-2.0), adapted to call through
//! `polish::claude_json` (Yap's existing structured-output Claude call) instead of
//! duplicating the request/error-handling logic, and to a flat (non-timestamped,
//! non-speaker-labeled) transcript, since Yap's local STT returns plain text rather than
//! open-granola's per-segment, per-speaker transcript.

use crate::polish::{api_key, claude_json};
use crate::store::{ActionItem, Chapter, NoteTemplate, Settings};
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;

/// The sentinel `template_id` meaning "work out which template fits, from the transcript
/// itself" rather than a specific one the user picked up front.
pub const AUTO: &str = "auto";

const NOTE_SYSTEM: &str = "\
You are Yap's note-structuring engine. Given a raw speech transcript from a continuous \
recording (a meeting, a feedback session, someone thinking out loud), produce STRICT \
JSON with keys: title, summary, chapters[{title,body}], decisions[], action_items[{text,owner}]. \
Rules: the transcript has no speaker labels, so never invent who said what unless they \
named themselves; prefer concrete facts and exact numbers; never invent content that \
wasn't said; keep chapter bodies under 60 words; split chapters by topic, not by time; \
decisions are things the speaker settled on, not things they're still weighing; an \
action item needs an owner only if the transcript names one, otherwise leave owner \
empty. \
A <template> may come with the transcript: it's the user's own description of the mode \
this note was recorded in (e.g. design feedback vs a generic meeting) and what to focus \
on or how to group things. Follow it for HOW you structure the note - but it never \
overrides the rule above against inventing content the transcript doesn't support. \
Output JSON only.";

fn note_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "summary": { "type": "string" },
            "chapters": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "body": { "type": "string" }
                    },
                    "required": ["title", "body"],
                    "additionalProperties": false
                }
            },
            "decisions": { "type": "array", "items": { "type": "string" } },
            "action_items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        "owner": { "type": "string" }
                    },
                    "required": ["text", "owner"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["title", "summary", "chapters", "decisions", "action_items"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
struct Structured {
    title: String,
    summary: String,
    chapters: Vec<ChapterOut>,
    decisions: Vec<String>,
    action_items: Vec<ActionItemOut>,
}

#[derive(Deserialize)]
struct ChapterOut {
    title: String,
    body: String,
}

#[derive(Deserialize)]
struct ActionItemOut {
    text: String,
    owner: String,
}

/// What `structure()` hands back to the caller - everything a `store::Note` needs except
/// the bookkeeping fields (id, created_at, raw, audio_secs, stt_ms) the pipeline fills in.
pub struct Note {
    pub title: String,
    pub summary: String,
    pub chapters: Vec<Chapter>,
    pub decisions: Vec<String>,
    pub action_items: Vec<ActionItem>,
    pub structure_ms: u64,
    pub engine: String,
    pub note: String,
    pub template_id: String,
}

/// Picks whichever template best fits the transcript, for when the user asked Yap to
/// decide rather than picking one up front. A small, cheap classification call (~100
/// tokens): the schema constrains the answer to one of the real template ids, so the
/// result is always usable even if Claude's reasoning about it is shaky. Falls back to the
/// first template (by convention, "default" is seeded first) if there's no API key, only
/// one template to choose from, or the call fails outright - auto-pick is a convenience,
/// never a reason to block structuring the note.
pub async fn classify(settings: &Settings, raw: &str, templates: &[NoteTemplate]) -> String {
    let fallback = || templates.first().map(|t| t.id.clone()).unwrap_or_default();
    if templates.len() <= 1 {
        return fallback();
    }
    let Some(key) = api_key(settings) else { return fallback() };

    let ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
    let options: String = templates.iter().map(|t| format!("- {}: {}\n", t.id, t.name)).collect();
    let system = vec![json!({ "type": "text", "text": format!(
        "Pick whichever template id best fits what kind of recording this transcript is. \
        Output STRICT JSON: {{\"templateId\": \"...\"}}. If nothing clearly fits, prefer \
        \"default\" when it's in the list, otherwise the first one.\n\nTemplates:\n{options}"
    )})];
    let schema = json!({
        "type": "object",
        "properties": { "templateId": { "type": "string", "enum": ids } },
        "required": ["templateId"],
        "additionalProperties": false
    });
    let user = format!("<transcript>\n{raw}\n</transcript>");
    match claude_json(settings, system, user, schema, 100, &key).await {
        Ok(text) => serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["templateId"].as_str().map(str::to_string))
            .unwrap_or_else(fallback),
        Err(_) => fallback(),
    }
}

/// Structures a transcript into a note, optionally guided by a named, user-authored
/// Markdown template describing the mode this note was recorded in (e.g. "Design
/// feedback") - `template_id`/`template_md` are both "" for the built-in default. Never
/// fails outright: a flaky Claude call or a missing API key must not lose a recording
/// that might be 45 minutes long, so this falls back to a plain-transcript note the same
/// way `polish::run` falls back to local rules.
pub async fn structure(settings: &Settings, raw: &str, template_id: &str, template_md: &str) -> Note {
    let started = Instant::now();
    let fallback = |reason: String| Note {
        title: fallback_title(raw),
        summary: raw.to_string(),
        chapters: vec![Chapter { title: "Transcript".into(), body: raw.to_string() }],
        decisions: Vec::new(),
        action_items: Vec::new(),
        structure_ms: started.elapsed().as_millis() as u64,
        engine: "raw transcript".into(),
        note: reason,
        template_id: template_id.to_string(),
    };

    let Some(key) = api_key(settings) else {
        return fallback("No Claude API key yet, so this note wasn't structured. Add one in Settings.".into());
    };
    let system = vec![json!({ "type": "text", "text": NOTE_SYSTEM })];
    let user = if template_md.trim().is_empty() {
        format!("<transcript>\n{raw}\n</transcript>")
    } else {
        format!("<template>\n{template_md}\n</template>\n\n<transcript>\n{raw}\n</transcript>")
    };
    match claude_json(settings, system, user, note_schema(), 4000, &key).await {
        Ok(text) => match serde_json::from_str::<Structured>(&text) {
            Ok(s) => Note {
                title: s.title,
                summary: s.summary,
                chapters: s.chapters.into_iter().map(|c| Chapter { title: c.title, body: c.body }).collect(),
                decisions: s.decisions,
                action_items: s
                    .action_items
                    .into_iter()
                    .map(|a| ActionItem { id: uuid::Uuid::new_v4().to_string(), text: a.text, owner: a.owner, done: false })
                    .collect(),
                structure_ms: started.elapsed().as_millis() as u64,
                engine: settings.claude_model.clone(),
                note: String::new(),
                template_id: template_id.to_string(),
            },
            Err(e) => fallback(format!("Couldn't read Claude's answer ({e}). Used the raw transcript instead.")),
        },
        Err(e) => fallback(format!("{e} Used the raw transcript instead.")),
    }
}

/// First handful of words of the transcript, as a placeholder title when there's no AI
/// summarization to produce a real one.
fn fallback_title(raw: &str) -> String {
    let words: Vec<&str> = raw.split_whitespace().take(8).collect();
    if words.is_empty() {
        return "Untitled note".into();
    }
    let mut title = words.join(" ");
    if raw.split_whitespace().count() > 8 {
        title.push('…');
    }
    let mut chars = title.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => title,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_api_key_falls_back_to_the_raw_transcript_without_losing_it() {
        let settings = Settings::default();
        let note = tauri::async_runtime::block_on(structure(
            &settings,
            "we decided to ship Friday and Sam will write the changelog",
            "",
            "",
        ));
        assert_eq!(note.engine, "raw transcript");
        assert!(note.note.contains("API key"));
        assert_eq!(note.summary, "we decided to ship Friday and Sam will write the changelog");
        assert_eq!(note.chapters.len(), 1);
        assert_eq!(note.chapters[0].body, note.summary);
        assert!(note.decisions.is_empty());
        assert!(note.action_items.is_empty());
    }

    #[test]
    fn fallback_title_shortens_and_capitalizes() {
        assert_eq!(fallback_title("we decided to ship on friday and sam will write the changelog this week"), "We decided to ship on friday and sam…");
        assert_eq!(fallback_title("short note"), "Short note");
        assert_eq!(fallback_title(""), "Untitled note");
    }
}
