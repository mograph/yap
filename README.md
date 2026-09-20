# Yap

Push-to-talk dictation that still sounds like you. Hold a key, talk, let go, and your words get typed into whatever app you're in, cleaned up the way *you* would clean them up, not polished into corporate email.

- **Hold ⌥ Option** to talk, let go to finish (or pick right Option, fn/🌐, or any key combo; Ctrl+Shift+Space on Windows)
- **Double-tap** for hands-free, tap again to finish, **Esc** cancels. Option still works normally in shortcuts and clicks
- Every dictation shows **what you said → what got typed**, what was changed and what was only suggested
- **Your voice** page: a raw↔polished slider, per-kind rules (*change it / suggest / leave it*), words that are yours, a say→write dictionary, and samples of how you write
- **Insights**: what Yap changed vs. what it could have, how much of your own wording survives, your go-to fillers

## How it works

```
hold key ─► mic (cpal) ─► Whisper on-device (whisper.cpp) ─► Claude + your voice profile ─► paste into focused app
                          or any OpenAI-compatible cloud STT     or offline local rules          (clipboard + ⌘V, then restored)
```

| Piece | Where | Notes |
|---|---|---|
| Hotkey, mic, paste | `src-tauri/src/lib.rs`, `audio.rs`, `paste.rs` | Tauri global shortcut, cpal, arboard + enigo |
| Speech → text | `src-tauri/src/stt.rs` | Local Whisper via whisper.cpp (Metal on Apple Silicon), or a cloud endpoint |
| Cleanup in your voice | `src-tauri/src/polish.rs` | The prompt lives in `INSTRUCTIONS`; your profile is appended per request |
| Settings, profile, history | `src-tauri/src/store.rs` | JSON in the app data dir (`~/Library/Application Support/io.tinkerstudio.yap`) |
| UI | `src/` (Svelte 5) | `App.svelte` main window, `Overlay.svelte` the floating pill |

## Speech models

Pick one under **Settings → Ears**. Everything runs on your computer after a one-time download. Whisper models run on whisper.cpp; the rest run on [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx).

| Model | Made by | Languages | Download | Good for |
|---|---|---|---|---|
| [Qwen3-ASR 0.6B](https://huggingface.co/Qwen/Qwen3-ASR-0.6B) | Alibaba | 52 + dialects | 879 MB | **Best multilingual.** Chinese, English, and mixing them |
| [Whisper Large v3 Turbo](https://huggingface.co/openai/whisper-large-v3-turbo) | OpenAI (open source) | 99 | 574 MB | **All-rounder** (the default). Keeps casual spellings like "kinda" |
| [Cohere Transcribe](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026) | Cohere | 14 | 1.7 GB | Top English accuracy. Set your language, it won't detect it |
| [Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) | NVIDIA | English + 24 European | 487 MB | **Fastest English** |
| [SenseVoice Small](https://github.com/FunAudioLLM/SenseVoice) | Alibaba | zh, yue, en, ja, ko | 166 MB | Tiny, instant Mandarin/Cantonese. Weak English |
| [FireRedASR2](https://github.com/FireRedTeam/FireRedASR) | Xiaohongshu | Mandarin + dialects, English | 839 MB | Mandarin specialist. Slow, no punctuation |
| Whisper Small / Base (English) | OpenAI (open source) | English | 488 / 148 MB | Older machines |

Measured on an M3 Max with clips spoken by macOS's built-in voices (clean audio; real voices will be messier). Seconds to transcribe:

| | English, 10 s | Chinese, 5 s | Chinese + English, 6 s | English, 33 s |
|---|---|---|---|---|
| Qwen3-ASR | 1.3 ✓ | 0.6 ✓ | 0.7 ✓ | 4.3 ✓ |
| Whisper Turbo | 1.0 ✓ | 0.8 ✓ | 0.9 ✓ (Traditional characters) | 1.4 ✓ |
| Cohere | 1.8 ✓ | ✗ (language set to English) | ✗ | 2.9 ✓ |
| Parakeet v3 | 0.4 ✓ | ✗ | ✗ | 1.4 ✓ |
| SenseVoice | 0.2 ✗ | 0.1 ✓ | 0.1 ~ | 0.6 ✗ |
| FireRedASR2 | 2.4 ✓ | 1.1 ✓ | 1.4 ✓ | 9.1 ✓ |

Run it yourself on any WAV clips (16 kHz mono) with `compare_models` in `src-tauri/src/tests.rs`.

## What talks to the internet

Nothing, unless you turn it on:

| When | Where it goes | What's sent |
|---|---|---|
| You click **Download** on a model | `huggingface.co` (Whisper models) or `github.com` (k2-fsa/sherpa-onnx releases, everything else) | Nothing; it's a one-time file download |
| Brain = **Claude** and you've added a key | `api.anthropic.com` | The transcript text + your voice profile. Never audio |
| Ears = **Cloud** and you've added a key | The endpoint you set | The audio clip |

Whisper is an open-source model from OpenAI, but running it locally does **not** call OpenAI. With local Whisper and "Local rules only", Yap is fully offline.

API keys are stored in `settings.json` in the app data folder (or read from `ANTHROPIC_API_KEY`).

## Run it

Needs Rust, Node + pnpm, and CMake (`brew install cmake`).

```sh
pnpm install
pnpm tauri dev          # run in development
pnpm tauri build        # build Yap.app / .dmg into src-tauri/target/release/bundle
```

In `pnpm tauri dev`, macOS attributes mic and Accessibility permissions to your terminal/editor. The built `Yap.app` asks for its own.

**UI-only preview** with fake data (no Rust needed): `pnpm dev`, then open http://127.0.0.1:1420/preview.html. Add `?view=voice|insights|settings`, `?onboarding=1`, or `?overlay=listening|thinking|done|error`.

**Tests:** `cd src-tauri && cargo test`. End-to-end Whisper test:
`YAP_TEST_MODEL=/path/ggml-base.en.bin YAP_TEST_WAV=/path/speech.wav cargo test -- --ignored --nocapture`
(make a WAV with `say -o speech.wav --file-format=WAVE --data-format=LEI16@16000 "um so like hi"`).

## Other platforms

- **Windows / Linux:** same code. Build on that machine with Rust, CMake and (Windows) the MSVC build tools, then `pnpm tauri build`. Not yet tested there.
- **iPhone:** a separate native app in `ios/`. See below.

## iPhone app (`ios/`)

A SwiftUI app plus a Yap keyboard, sharing settings, voice profile and history through an App Group (`group.io.tinkerstudio.yap`). The JSON matches the Mac app's files.

- **Ears:** Whisper on the iPhone via [WhisperKit](https://github.com/argmaxinc/WhisperKit) (Turbo 632 MB, Small, or Base). No Apple speech model and no cloud transcription: your voice never leaves the phone. Whisper only uses the Neural Engine and CPU, never the GPU, because iOS blocks GPU work from the background.
- **Brain:** the same Claude prompt and offline rules as the Mac (`Yap/Polish.swift` mirrors `src-tauri/src/polish.rs`).
- **Keyboard:** iOS keyboards can't use the microphone. The keyboard's mic key opens `yap://dictate` the first time; the app turns the mic on and keeps its audio session alive in the background for 5 minutes. After that the keyboard sends start/stop through Darwin notifications and types the result it reads from shared defaults.

Build it:

```sh
cd ios
xcodegen generate          # project.yml -> Yap.xcodeproj
open Yap.xcodeproj         # pick your iPhone, press Run
```

For a real iPhone: plug it in, turn on Settings → Privacy & Security → Developer Mode, and let Xcode sign with your team. Then on the phone: Settings → Yap → Keyboards → turn on Yap and Allow Full Access.

Test without a mic: launch with `-testWav <path to 16 kHz mono wav> -testModel openai_whisper-base.en`, and it downloads the model and transcribes the file.
