// Dev-only browser preview with fake data, so the UI can be designed without the Rust side.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import Overlay from "./Overlay.svelte";
import type { Dictation, Edit, Kind, Snapshot } from "./lib/api";
import { app, type View } from "./lib/state.svelte";

const params = new URLSearchParams(location.search);
const ago = (hours: number) => new Date(Date.now() - hours * 3600e3).toISOString();

let seed = 7;
const rand = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646;
const pick = <T>(xs: T[]) => xs[Math.floor(rand() * xs.length)];

const history: Dictation[] = [
  {
    id: "1",
    createdAt: ago(0.05),
    raw: "um so yeah i'm gonna be like, uh, ten minutes late, traffic is kinda insane rn lol",
    text: "So yeah, I'm gonna be like ten minutes late, traffic is kinda insane rn lol",
    edits: [
      { original: "um", replacement: "", kind: "filler", applied: true, why: "just padding" },
      { original: "uh,", replacement: "", kind: "filler", applied: true, why: "just padding" },
      { original: "so yeah i'm", replacement: "So yeah, I'm", kind: "punctuation", applied: true, why: "capital and a comma" },
      { original: "rn", replacement: "right now", kind: "slang", applied: false, why: "could spell it out" },
    ],
    engine: "claude-opus-5",
    note: "",
    words: 15,
    keptPct: 100,
    audioSecs: 5.2,
    sttMs: 410,
    polishMs: 1320,
  },
  {
    id: "2",
    createdAt: ago(1.6),
    raw: "hey can you send me the deck for tinker studio by thursday no wait wednesday",
    text: "Hey, can you send me the deck for Tinker Studio by Wednesday?",
    edits: [
      { original: "thursday no wait wednesday", replacement: "Wednesday", kind: "correction", applied: true, why: "you corrected yourself" },
      { original: "tinker studio", replacement: "Tinker Studio", kind: "dictionary", applied: true, why: "your dictionary" },
      { original: "hey can", replacement: "Hey, can", kind: "punctuation", applied: true, why: "comma after the greeting" },
    ],
    engine: "claude-opus-5",
    note: "",
    words: 12,
    keptPct: 86,
    audioSecs: 4.4,
    sttMs: 380,
    polishMs: 1100,
  },
  {
    id: "3",
    createdAt: ago(5),
    raw: "ok so the plan is we ship the beta friday and me and jess was thinking we do the launch thread monday",
    text: "Ok so the plan is we ship the beta Friday, and me and Jess was thinking we do the launch thread Monday.",
    edits: [
      { original: "me and jess was", replacement: "Jess and I were", kind: "grammar", applied: false, why: "subject and verb don't agree" },
      { original: "friday and", replacement: "Friday, and", kind: "punctuation", applied: true, why: "comma between clauses" },
    ],
    engine: "local rules",
    note: "No Claude API key yet, so this used local rules. Add one in Settings.",
    words: 21,
    keptPct: 100,
    audioSecs: 6.8,
    sttMs: 520,
    polishMs: 3,
  },
];

const PHRASES = [
  "Honestly I think we should just ship it and fix the rest later.",
  "Can you grab oat milk on the way back? And the good bread.",
  "Lowkey loving the new homepage, the colors hit different.",
  "Running five minutes behind, start without me.",
  "Let's move standup to 10 tomorrow so we can finish the review.",
  "That demo was so good, send me the recording when you can.",
];
const KIND_ODDS: [Kind, number, boolean][] = [
  ["filler", 0.9, true],
  ["punctuation", 0.8, true],
  ["correction", 0.35, true],
  ["grammar", 0.3, false],
  ["rephrase", 0.25, false],
  ["formatting", 0.12, true],
  ["dictionary", 0.2, true],
  ["slang", 0.15, false],
];
const FILLERS = ["um", "uh", "like", "you know", "I mean", "um", "uh", "so"];
for (let i = 0; i < 46; i++) {
  const edits: Edit[] = [];
  for (const [kind, odds, applied] of KIND_ODDS) {
    const n = rand() < odds ? 1 + Math.floor(rand() * (kind === "filler" ? 4 : 2)) : 0;
    for (let k = 0; k < n; k++)
      edits.push({ original: kind === "filler" ? pick(FILLERS) : "…", replacement: "", kind, applied, why: "" });
  }
  const text = pick(PHRASES);
  const words = text.split(" ").length;
  history.push({
    id: `g${i}`,
    createdAt: ago(8 + rand() * 24 * 29),
    raw: text.toLowerCase(),
    text,
    edits,
    engine: "claude-opus-5",
    note: "",
    words,
    keptPct: 78 + rand() * 22,
    audioSecs: words / 2.6,
    sttMs: 400,
    polishMs: 1200,
  });
}
history.sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt));

/** `?platform=windows` previews the Windows build's talk keys and permission copy. */
const platform = params.get("platform") ?? "macos";

const snapshot: Snapshot = {
  settings: {
    trigger: platform === "macos" ? "option" : "right-ctrl",
    shortcut: platform === "macos" ? "Alt+Space" : "Ctrl+Shift+Space",
    sttEngine: "local",
    localModel: "large-v3-turbo-q5_0",
    language: "auto",
    cloudSttUrl: "https://api.groq.com/openai/v1/audio/transcriptions",
    cloudSttModel: "whisper-large-v3-turbo",
    cloudSttKey: "",
    brain: "claude",
    claudeModel: "claude-opus-5",
    anthropicKey: params.get("onboarding") ? "" : "sk-ant-preview",
    autoPaste: true,
    restoreClipboard: true,
    sounds: true,
    onboarded: !params.get("onboarding"),
    lists: "ask",
    libraryFolder: "",
    cloudSync: false,
    firebaseProjectId: "",
    firebaseApiKey: "",
    googleClientId: "",
    googleClientSecret: "",
  },
  profile: {
    name: "",
    tone: 25,
    casual: false,
    rules: { filler: "do", correction: "do", punctuation: "do", grammar: "suggest", slang: "leave", rephrase: "suggest", swearing: "leave", formatting: "do" },
    myWords: ["gonna", "wanna", "kinda", "lowkey"],
    dictionary: [
      { say: "tinker studio", write: "Tinker Studio" },
      { say: "claude code", write: "Claude Code" },
    ],
    samples: "",
    notes: "",
    updatedAt: 0,
  },
  history: params.get("empty") ? [] : history,
  models: [
    { id: "qwen3-asr-0.6b", label: "Qwen3-ASR 0.6B", maker: "Alibaba · Qwen", languages: "52 languages & dialects", group: "Lots of languages", sizeMb: 879, note: "Aced every test here, including Chinese and mixed Chinese–English. Tends to tidy slang a little (“kinda” → “kind of”).", badge: "Best multilingual", link: "https://huggingface.co/Qwen/Qwen3-ASR-0.6B", languageMode: "auto", usesVocabulary: true, installed: false },
    { id: "large-v3-turbo-q5_0", label: "Whisper Large v3 Turbo", maker: "OpenAI (open source)", languages: "99 languages", group: "Lots of languages", sizeMb: 574, note: "The all-rounder. Keeps casual spellings like “kinda”, fast on long dictations, widest language list.", badge: "All-rounder", link: "https://huggingface.co/openai/whisper-large-v3-turbo", languageMode: "any", usesVocabulary: true, installed: !params.get("onboarding") },
    { id: "cohere-transcribe", label: "Cohere Transcribe", maker: "Cohere", languages: "14 languages", group: "Lots of languages", sizeMb: 1700, note: "Top of the 2026 accuracy leaderboard for English. It can't tell which language you're speaking, so set yours below.", badge: "", link: "https://huggingface.co/CohereLabs/cohere-transcribe-03-2026", languageMode: "pick", usesVocabulary: false, installed: false },
    { id: "parakeet-tdt-0.6b-v3", label: "Parakeet TDT 0.6B v3", maker: "NVIDIA", languages: "English + 24 European languages", group: "English & European", sizeMb: 487, note: "Fastest by far: 10 seconds of English in under half a second, and very accurate. Can't do Chinese.", badge: "Fastest English", link: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3", languageMode: "auto", usesVocabulary: false, installed: false },
    { id: "sensevoice-small", label: "SenseVoice Small", maker: "Alibaba · FunAudioLLM", languages: "Mandarin, Cantonese, English, Japanese, Korean", group: "Chinese specialists", sizeMb: 166, note: "Tiny and instant for Mandarin and Cantonese. Weak at English.", badge: "", link: "https://github.com/FunAudioLLM/SenseVoice", languageMode: "auto", usesVocabulary: false, installed: false },
    { id: "fireredasr2", label: "FireRedASR2", maker: "Xiaohongshu · FireRed", languages: "Mandarin, Chinese dialects, English", group: "Chinese specialists", sizeMb: 839, note: "Top Mandarin benchmark scores, dialects included. Slower, and leaves punctuation to the cleanup step.", badge: "", link: "https://github.com/FireRedTeam/FireRedASR", languageMode: "auto", usesVocabulary: false, installed: false },
    { id: "small.en", label: "Whisper Small · English", maker: "OpenAI (open source)", languages: "English", group: "Small & light", sizeMb: 488, note: "Middle ground for older machines.", badge: "", link: "https://huggingface.co/openai/whisper-small.en", languageMode: "auto", usesVocabulary: true, installed: false },
    { id: "base.en", label: "Whisper Base · English", maker: "OpenAI (open source)", languages: "English", group: "Small & light", sizeMb: 148, note: "Tiny and quick, less accurate.", badge: "", link: "https://huggingface.co/openai/whisper-base.en", languageMode: "auto", usesVocabulary: true, installed: false },
  ],
  platform,
  accessibility: true,
  envKey: false,
};

mockWindows("main");
mockIPC(
  (cmd, args) => {
    switch (cmd) {
      case "get_snapshot":
        return structuredClone(snapshot);
      case "get_settings":
        return snapshot.settings;
      case "check_accessibility":
        return true;
      case "polish_text":
        return { ...history[0], id: "try", raw: (args as { raw: string }).raw };
      default:
        return null;
    }
  },
  { shouldMockEvents: true },
);

const overlay = params.get("overlay");
if (overlay) {
  const target = document.getElementById("app")!;
  target.style.cssText = "position:fixed;inset:0;background:linear-gradient(135deg,#6d83f2,#f5a3c7 60%,#ffd29d)";
  mount(Overlay, { target });
  const messages: Record<string, string> = {
    thinking: "Making it sound like you…",
    done: "Pasted",
    choose: "- Today I fixed the login bug\n- Shipped the icons\n- Merged everything",
    error: "Claude rejected the API key. Check it in Settings.",
  };
  const texts: Record<string, string> = {
    done: "So yeah, I'm gonna be like ten minutes late, traffic is kinda insane rn lol",
    choose: "Today I fixed the login bug, shipped the icons, and merged everything.",
  };
  setTimeout(() => {
    emit("yap://state", { phase: overlay, message: messages[overlay] ?? "", words: 23, locked: params.has("locked"), text: texts[overlay] ?? "" });
    if (overlay === "listening") setInterval(() => emit("yap://level", 0.004 + rand() ** 2 * 0.12), 40);
  }, 200);
} else {
  app.view = (params.get("view") as View) ?? "home";
  mount(App, { target: document.getElementById("app")! });
}
