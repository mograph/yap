//! Speech-to-text. Local engines: whisper.cpp for Whisper models, sherpa-onnx for
//! Qwen3-ASR, Parakeet, SenseVoice, FireRedASR2 and Cohere Transcribe. Or any
//! OpenAI-compatible transcription API in the cloud.

use crate::store::{Profile, Settings};
use futures_util::StreamExt;
use regex::Regex;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Whisper,
    Qwen3,
    Parakeet,
    SenseVoice,
    FireRed,
    Cohere,
}

pub struct Spec {
    pub id: &'static str,
    pub label: &'static str,
    pub maker: &'static str,
    pub languages: &'static str,
    pub group: &'static str,
    pub size_mb: u32,
    pub note: &'static str,
    pub badge: &'static str,
    /// The model's own page, for people who want to read up on it.
    pub link: &'static str,
    pub family: Family,
    /// Whisper: the ggml file stem. sherpa-onnx: the release archive and folder name.
    pub file: &'static str,
}

const WHISPER_FILES: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
const SHERPA_RELEASES: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models";

pub const CATALOG: &[Spec] = &[
    Spec {
        id: "qwen3-asr-0.6b",
        label: "Qwen3-ASR 0.6B",
        maker: "Alibaba · Qwen",
        languages: "52 languages & dialects",
        group: "Lots of languages",
        size_mb: 879,
        note: "Aced every test here, including Chinese and mixed Chinese–English. Tends to tidy slang a little (“kinda” → “kind of”).",
        badge: "Best multilingual",
        link: "https://huggingface.co/Qwen/Qwen3-ASR-0.6B",
        family: Family::Qwen3,
        file: "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
    },
    Spec {
        id: "large-v3-turbo-q5_0",
        label: "Whisper Large v3 Turbo",
        maker: "OpenAI (open source)",
        languages: "99 languages",
        group: "Lots of languages",
        size_mb: 574,
        note: "The all-rounder. Keeps casual spellings like “kinda”, fast on long dictations, widest language list.",
        badge: "All-rounder",
        link: "https://huggingface.co/openai/whisper-large-v3-turbo",
        family: Family::Whisper,
        file: "large-v3-turbo-q5_0",
    },
    Spec {
        id: "cohere-transcribe",
        label: "Cohere Transcribe",
        maker: "Cohere",
        languages: "14 languages",
        group: "Lots of languages",
        size_mb: 1700,
        note: "Top of the 2026 accuracy leaderboard for English. It can't tell which language you're speaking, so set yours below.",
        badge: "",
        link: "https://huggingface.co/CohereLabs/cohere-transcribe-03-2026",
        family: Family::Cohere,
        file: "sherpa-onnx-cohere-transcribe-14-lang-int8-2026-04-01",
    },
    Spec {
        id: "parakeet-tdt-0.6b-v3",
        label: "Parakeet TDT 0.6B v3",
        maker: "NVIDIA",
        languages: "English + 24 European languages",
        group: "English & European",
        size_mb: 487,
        note: "Fastest by far: 10 seconds of English in under half a second, and very accurate. Can't do Chinese.",
        badge: "Fastest English",
        link: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3",
        family: Family::Parakeet,
        file: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
    },
    Spec {
        id: "sensevoice-small",
        label: "SenseVoice Small",
        maker: "Alibaba · FunAudioLLM",
        languages: "Mandarin, Cantonese, English, Japanese, Korean",
        group: "Chinese specialists",
        size_mb: 166,
        note: "Tiny and instant for Mandarin and Cantonese. Weak at English.",
        badge: "",
        link: "https://github.com/FunAudioLLM/SenseVoice",
        family: Family::SenseVoice,
        file: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
    },
    Spec {
        id: "fireredasr2",
        label: "FireRedASR2",
        maker: "Xiaohongshu · FireRed",
        languages: "Mandarin, Chinese dialects, English",
        group: "Chinese specialists",
        size_mb: 839,
        note: "Top Mandarin benchmark scores, dialects included. Slower, and leaves punctuation to the cleanup step.",
        badge: "",
        link: "https://github.com/FireRedTeam/FireRedASR",
        family: Family::FireRed,
        file: "sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26",
    },
    Spec {
        id: "small.en",
        label: "Whisper Small · English",
        maker: "OpenAI (open source)",
        languages: "English",
        group: "Small & light",
        size_mb: 488,
        note: "Middle ground for older machines.",
        badge: "",
        link: "https://huggingface.co/openai/whisper-small.en",
        family: Family::Whisper,
        file: "small.en",
    },
    Spec {
        id: "base.en",
        label: "Whisper Base · English",
        maker: "OpenAI (open source)",
        languages: "English",
        group: "Small & light",
        size_mb: 148,
        note: "Tiny and quick, less accurate.",
        badge: "",
        link: "https://huggingface.co/openai/whisper-base.en",
        family: Family::Whisper,
        file: "base.en",
    },
];

pub fn spec(id: &str) -> Option<&'static Spec> {
    CATALOG.iter().find(|s| s.id == id)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub maker: &'static str,
    pub languages: &'static str,
    pub group: &'static str,
    pub size_mb: u32,
    pub note: &'static str,
    pub badge: &'static str,
    pub link: &'static str,
    /// "any": pick from all languages, "pick": must be told, "auto": figures it out itself
    pub language_mode: &'static str,
    /// Whether it uses the names and slang from Your voice while listening
    pub uses_vocabulary: bool,
    pub installed: bool,
}

pub fn models(data_dir: &Path) -> Vec<ModelInfo> {
    CATALOG
        .iter()
        .map(|s| ModelInfo {
            id: s.id,
            label: s.label,
            maker: s.maker,
            languages: s.languages,
            group: s.group,
            size_mb: s.size_mb,
            note: s.note,
            badge: s.badge,
            link: s.link,
            language_mode: match s.family {
                Family::Whisper if !s.file.ends_with(".en") => "any",
                Family::Cohere => "pick",
                _ => "auto",
            },
            uses_vocabulary: matches!(s.family, Family::Whisper | Family::Qwen3),
            installed: installed(data_dir, s),
        })
        .collect()
}

/// Marks a sherpa-onnx model folder as fully unpacked.
const READY: &str = ".yap-ready";

pub fn location(data_dir: &Path, spec: &Spec) -> PathBuf {
    let dir = data_dir.join("models");
    match spec.family {
        Family::Whisper => dir.join(format!("ggml-{}.bin", spec.file)),
        _ => dir.join(spec.file),
    }
}

fn installed(data_dir: &Path, spec: &Spec) -> bool {
    let path = location(data_dir, spec);
    match spec.family {
        Family::Whisper => path.exists(),
        _ => path.join(READY).exists(),
    }
}

pub fn delete(data_dir: &Path, id: &str) -> Result<(), String> {
    let spec = spec(id).ok_or_else(|| format!("Unknown model {id}"))?;
    let path = location(data_dir, spec);
    match spec.family {
        Family::Whisper => std::fs::remove_file(path),
        _ => std::fs::remove_dir_all(path),
    }
    .map_err(|e| e.to_string())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    id: &'a str,
    received: u64,
    total: u64,
    /// "downloading" or "unpacking"
    stage: &'a str,
    done: bool,
}

pub async fn download(app: &AppHandle, data_dir: &Path, id: &str) -> Result<(), String> {
    let spec = spec(id).ok_or_else(|| format!("Unknown model {id}"))?;
    let dir = data_dir.join("models");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let url = match spec.family {
        Family::Whisper => format!("{WHISPER_FILES}/ggml-{}.bin", spec.file),
        _ => format!("{SHERPA_RELEASES}/{}.tar.bz2", spec.file),
    };
    let resp = reqwest::get(&url)
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Download failed: {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    let part = dir.join(format!("{}.part", spec.id));
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    let mut received = 0u64;
    let mut last = Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download interrupted: {e}"))?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        received += chunk.len() as u64;
        if last.elapsed() > Duration::from_millis(120) {
            let _ = app.emit("yap://download", Progress { id, received, total, stage: "downloading", done: false });
            last = Instant::now();
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);

    let dest = location(data_dir, spec);
    if spec.family == Family::Whisper {
        std::fs::rename(&part, &dest).map_err(|e| e.to_string())?;
    } else {
        let _ = app.emit("yap://download", Progress { id, received, total, stage: "unpacking", done: false });
        tauri::async_runtime::spawn_blocking(move || unpack(&part, &dir, &dest))
            .await
            .map_err(|e| e.to_string())??;
    }
    let _ = app.emit("yap://download", Progress { id, received, total, stage: "done", done: true });
    Ok(())
}

fn unpack(archive: &Path, into: &Path, dest: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dest);
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    tar::Archive::new(bzip2::read::BzDecoder::new(std::io::BufReader::new(file)))
        .unpack(into)
        .map_err(|e| format!("Couldn't unpack the model: {e}"))?;
    if !dest.is_dir() {
        return Err("The download didn't contain the model folder it should have.".into());
    }
    std::fs::write(dest.join(READY), b"").map_err(|e| e.to_string())?;
    std::fs::remove_file(archive).map_err(|e| e.to_string())
}

/// The user's names and slang, used to bias recognition where the model supports it.
pub fn vocabulary(profile: &Profile) -> String {
    let mut terms: Vec<&str> = profile
        .dictionary
        .iter()
        .map(|d| d.write.trim())
        .chain(profile.my_words.iter().map(|w| w.trim()))
        .filter(|t| !t.is_empty())
        .collect();
    terms.dedup();
    terms.join(", ")
}

/// Whisper's initial prompt: a casual lead-in plus the user's vocabulary.
pub fn whisper_prompt(vocabulary: &str) -> String {
    if vocabulary.is_empty() {
        String::new()
    } else {
        format!("Okay so, like, {vocabulary}.")
    }
}

static ANNOTATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\[[^\]]*\]|\((?:silence|music|inaudible|laughs|laughing|applause|coughs?|sighs?|background noise|blank_audio)\)")
        .unwrap()
});
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Strips whisper's [BLANK_AUDIO]-style annotations.
pub fn tidy(text: &str) -> String {
    let t = ANNOTATION.replace_all(text, " ");
    SPACES.replace_all(t.trim(), " ").into_owned()
}

/// Splits long audio into pieces of at most `max_secs`, cutting at the quietest moment
/// near the end of each piece so words aren't chopped in half.
pub fn split(samples: &[f32], max_secs: f32) -> Vec<&[f32]> {
    const WIN: usize = 1600; // 100 ms
    let max = (max_secs * 16_000.0) as usize;
    let energy = |i: usize| samples[i..i + WIN].iter().map(|s| s * s).sum::<f32>();
    let mut pieces = Vec::new();
    let mut start = 0;
    while samples.len() - start > max {
        let (lo, hi) = (start + max * 2 / 3, start + max - WIN);
        let cut = (lo..hi)
            .step_by(WIN / 2)
            .min_by(|a, b| energy(*a).total_cmp(&energy(*b)))
            .map_or(start + max, |i| i + WIN / 2);
        pieces.push(&samples[start..cut]);
        start = cut;
    }
    pieces.push(&samples[start..]);
    pieces
}

/// Some models (FireRedASR, SenseVoice) write English in ALL CAPS. Lowercase it and let
/// the cleanup step handle capitals like it would for anything else.
pub fn unshout(text: String) -> String {
    let shouting = text.chars().any(char::is_uppercase) && !text.chars().any(char::is_lowercase);
    if shouting {
        text.to_lowercase()
    } else {
        text
    }
}

#[cfg(desktop)]
pub use local::Local;

#[cfg(desktop)]
mod local {
    use super::{location, spec, split, tidy, unshout, whisper_prompt, Family, Spec};
    use sherpa_onnx::{
        OfflineCohereTranscribeModelConfig, OfflineFireRedAsrModelConfig, OfflineQwen3ASRModelConfig,
        OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig, OfflineTransducerModelConfig,
    };
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    enum Engine {
        Whisper(WhisperContext),
        Sherpa(OfflineRecognizer),
    }

    /// Keeps the one model you're using loaded, so each dictation starts instantly.
    #[derive(Default)]
    pub struct Local {
        loaded: Mutex<Option<(String, Arc<Engine>)>>,
    }

    fn threads() -> i32 {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8) as i32
    }

    impl Local {
        fn engine(&self, data_dir: &Path, spec: &Spec, vocabulary: &str) -> Result<Arc<Engine>, String> {
            // Qwen3 takes the vocabulary at load time, so a changed profile means a reload.
            let key = match spec.family {
                Family::Qwen3 => format!("{}|{vocabulary}", spec.id),
                _ => spec.id.to_string(),
            };
            let mut guard = self.loaded.lock().unwrap();
            if let Some((k, engine)) = guard.as_ref() {
                if *k == key {
                    return Ok(engine.clone());
                }
            }
            *guard = None; // free the old model before loading the next one
            let path = location(data_dir, spec);
            if !path.exists() {
                return Err(format!("{} isn't downloaded yet. Grab it in Settings.", spec.label));
            }
            let engine = Arc::new(load(spec, &path, vocabulary)?);
            *guard = Some((key, engine.clone()));
            Ok(engine)
        }

        pub fn preload(&self, data_dir: &Path, id: &str, vocabulary: &str) {
            if let Some(spec) = spec(id) {
                let _ = self.engine(data_dir, spec, vocabulary);
            }
        }

        pub fn transcribe(
            &self,
            data_dir: &Path,
            id: &str,
            samples: &[f32],
            language: &str,
            vocabulary: &str,
        ) -> Result<String, String> {
            let spec = spec(id).ok_or_else(|| format!("Unknown model {id}"))?;
            let engine = self.engine(data_dir, spec, vocabulary)?;
            let text = match &*engine {
                Engine::Whisper(ctx) => whisper(ctx, spec, samples, language, &whisper_prompt(vocabulary))?,
                // sherpa-onnx models are happiest with under ~30 s at a time
                Engine::Sherpa(rec) => unshout(
                    split(samples, 25.0)
                        .into_iter()
                        .map(|piece| sherpa(rec, spec, piece, language))
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
            };
            Ok(tidy(&text))
        }
    }

    /// Finds the model file whose name contains every part, preferring int8 builds.
    fn find(dir: &Path, part: &str) -> Result<Option<String>, String> {
        let mut hits: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                let name = p.file_name().unwrap_or_default().to_string_lossy();
                name.ends_with(".onnx") && name.contains(part)
            })
            .collect();
        hits.sort_by_key(|p| !p.to_string_lossy().contains("int8"));
        match hits.first() {
            Some(p) => Ok(Some(p.to_string_lossy().into_owned())),
            None => Err(format!("The model folder is missing its {part} file. Try downloading it again.")),
        }
    }

    fn load(spec: &Spec, path: &Path, vocabulary: &str) -> Result<Engine, String> {
        if spec.family == Family::Whisper {
            return WhisperContext::new_with_params(&path.to_string_lossy(), WhisperContextParameters::default())
                .map(Engine::Whisper)
                .map_err(|e| format!("Couldn't load {}: {e}", spec.label));
        }
        let tokens = Some(path.join("tokens.txt").to_string_lossy().into_owned());
        let mut config = OfflineRecognizerConfig::default();
        let m = &mut config.model_config;
        m.num_threads = threads();
        m.provider = Some("cpu".into());
        match spec.family {
            Family::Parakeet => {
                m.transducer = OfflineTransducerModelConfig {
                    encoder: find(path, "encoder")?,
                    decoder: find(path, "decoder")?,
                    joiner: find(path, "joiner")?,
                };
                m.model_type = Some("nemo_transducer".into());
                m.tokens = tokens;
            }
            Family::SenseVoice => {
                m.sense_voice = OfflineSenseVoiceModelConfig {
                    model: find(path, "model")?,
                    language: Some("auto".into()),
                    use_itn: true,
                };
                m.tokens = tokens;
            }
            Family::Qwen3 => {
                m.qwen3_asr = OfflineQwen3ASRModelConfig {
                    conv_frontend: find(path, "conv_frontend")?,
                    encoder: find(path, "encoder")?,
                    decoder: find(path, "decoder")?,
                    tokenizer: Some(path.join("tokenizer").to_string_lossy().into_owned()),
                    // room for a full 25 s piece of fast talking
                    max_total_len: 1536,
                    max_new_tokens: 512,
                    hotwords: (!vocabulary.is_empty()).then(|| vocabulary.to_string()),
                    ..Default::default()
                };
                m.tokens = Some(String::new());
            }
            Family::FireRed => {
                m.fire_red_asr = OfflineFireRedAsrModelConfig { encoder: find(path, "encoder")?, decoder: find(path, "decoder")? };
                m.tokens = tokens;
            }
            Family::Cohere => {
                m.cohere_transcribe = OfflineCohereTranscribeModelConfig {
                    encoder: find(path, "encoder")?,
                    decoder: find(path, "decoder")?,
                    use_punct: true,
                    use_itn: true,
                    ..Default::default()
                };
                m.tokens = tokens;
            }
            Family::Whisper => unreachable!(),
        }
        OfflineRecognizer::create(&config)
            .map(Engine::Sherpa)
            .ok_or_else(|| format!("Couldn't load {}. Try downloading it again.", spec.label))
    }

    fn sherpa(rec: &OfflineRecognizer, spec: &Spec, samples: &[f32], language: &str) -> String {
        let stream = rec.create_stream();
        if spec.family == Family::Cohere {
            stream.set_option("language", if language == "auto" { "en" } else { language });
        }
        // A little trailing silence helps the last word come through.
        let mut audio = samples.to_vec();
        audio.extend(std::iter::repeat_n(0.0, 6_400));
        stream.accept_waveform(16_000, &audio);
        rec.decode(&stream);
        stream.get_result().map(|r| r.text).unwrap_or_default()
    }

    fn whisper(ctx: &WhisperContext, spec: &Spec, samples: &[f32], language: &str, prompt: &str) -> Result<String, String> {
        let mut state = ctx.create_state().map_err(|e| e.to_string())?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        let lang = if spec.file.ends_with(".en") { "en" } else { language };
        params.set_language(Some(lang));
        params.set_n_threads(threads());
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_no_context(true);
        params.set_suppress_blank(true);
        if !prompt.is_empty() {
            params.set_initial_prompt(prompt);
        }
        // whisper.cpp wants at least a second of audio
        let mut audio = samples.to_vec();
        if audio.len() < 16_000 {
            audio.resize(16_000, 0.0);
        }
        state.full(params, &audio).map_err(|e| format!("Whisper failed: {e}"))?;
        let n = state.full_n_segments().map_err(|e| e.to_string())?;
        let mut out = String::new();
        for i in 0..n {
            out.push_str(&state.full_get_segment_text(i).map_err(|e| e.to_string())?);
        }
        Ok(out)
    }
}

pub async fn transcribe_cloud(settings: &Settings, samples: &[f32], prompt: &str) -> Result<String, String> {
    if settings.cloud_stt_key.trim().is_empty() {
        return Err("Add your transcription API key in Settings, or switch to a model on this device.".into());
    }
    let part = reqwest::multipart::Part::bytes(wav(samples))
        .file_name("speech.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let mut form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", settings.cloud_stt_model.clone())
        .text("response_format", "json");
    if !prompt.is_empty() {
        form = form.text("prompt", prompt.to_string());
    }
    if settings.language != "auto" {
        form = form.text("language", settings.language.clone());
    }
    let resp = reqwest::Client::new()
        .post(&settings.cloud_stt_url)
        .bearer_auth(settings.cloud_stt_key.trim())
        .multipart(form)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("Transcription service unreachable: {e}"))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let msg = body["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(format!("Transcription failed ({status}): {msg}"));
    }
    Ok(tidy(body["text"].as_str().unwrap_or_default()))
}

/// 16-bit PCM mono WAV at 16 kHz.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&16_000u32.to_le_bytes());
    out.extend_from_slice(&32_000u32.to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    out
}
