import { invoke } from "@tauri-apps/api/core";

export type Rule = "do" | "suggest" | "leave";
export type Kind =
  | "filler"
  | "correction"
  | "punctuation"
  | "grammar"
  | "slang"
  | "rephrase"
  | "swearing"
  | "formatting"
  | "dictionary";

export interface Settings {
  /** "option" | "right-option" | "fn" (macOS) | "shortcut" */
  trigger: string;
  shortcut: string;
  sttEngine: "local" | "cloud";
  localModel: string;
  language: string;
  cloudSttUrl: string;
  cloudSttModel: string;
  cloudSttKey: string;
  brain: "claude" | "local";
  claudeModel: string;
  anthropicKey: string;
  autoPaste: boolean;
  restoreClipboard: boolean;
  sounds: boolean;
  onboarded: boolean;
  /** When there's a tidier arrangement on offer: ask, always take it, or never */
  lists: "ask" | "auto" | "never";
  /** A synced folder holding yap-library.json, or "" */
  libraryFolder: string;
  /** Keep an encrypted copy of the library in Firestore too */
  cloudSync: boolean;
  firebaseProjectId: string;
  firebaseApiKey: string;
  /** Only needed to sign in with Google */
  googleClientId: string;
  googleClientSecret: string;
}

export interface DictEntry {
  say: string;
  write: string;
}

export interface Profile {
  name: string;
  tone: number;
  /** Write it like a Slack message: no structure imposed, never more formal than chatty */
  casual: boolean;
  rules: Partial<Record<Kind, Rule>>;
  myWords: string[];
  dictionary: DictEntry[];
  samples: string;
  notes: string;
  updatedAt: number;
}

export interface Edit {
  original: string;
  replacement: string;
  kind: Kind;
  applied: boolean;
  why: string;
}

export interface Dictation {
  id: string;
  createdAt: string;
  raw: string;
  text: string;
  edits: Edit[];
  engine: string;
  note: string;
  words: number;
  keptPct: number;
  audioSecs: number;
  sttMs: number;
  polishMs: number;
  /** A tidier arrangement you can switch to: a bullet list, or your subjects gathered up */
  list?: string;
}

export interface ModelInfo {
  id: string;
  label: string;
  maker: string;
  languages: string;
  group: string;
  sizeMb: number;
  note: string;
  badge: string;
  link: string;
  /** "any": pick from all languages, "pick": must be told, "auto": figures it out itself */
  languageMode: "any" | "pick" | "auto";
  usesVocabulary: boolean;
  installed: boolean;
}

/** Cloud sync state. Never carries the passphrase or the token. */
export interface CloudStatus {
  /** This computer is registered with Firebase, one way or the other. */
  registered: boolean;
  /** Registered without anyone signing in: the passphrase is the identity. */
  anonymous: boolean;
  /** Empty unless signed in with Google. */
  email: string;
  hasPassphrase: boolean;
  minPassphrase: number;
}

export interface Snapshot {
  settings: Settings;
  profile: Profile;
  history: Dictation[];
  models: ModelInfo[];
  platform: string;
  accessibility: boolean;
  envKey: boolean;
}

export interface PhaseEvent {
  phase: "idle" | "listening" | "thinking" | "choose" | "done" | "error";
  /** Status text, or the list version while choosing */
  message: string;
  words: number;
  locked: boolean;
  /** What got typed ("done"), or the as-said version ("choose") */
  text: string;
}

export interface DownloadEvent {
  id: string;
  received: number;
  total: number;
  stage: "downloading" | "unpacking" | "done";
  done: boolean;
}

export const api = {
  snapshot: () => invoke<Snapshot>("get_snapshot"),
  settings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  saveProfile: (profile: Profile) => invoke<void>("save_profile", { profile }),
  updateDictation: (item: Dictation) => invoke<void>("update_dictation", { item }),
  deleteDictation: (id: string) => invoke<void>("delete_dictation", { id }),
  clearHistory: () => invoke<void>("clear_history"),
  exportLibrary: (path: string) => invoke<void>("export_library", { path }),
  importLibrary: (path: string) => invoke<{ words: number; dictations: number }>("import_library", { path }),
  setLibraryFolder: (folder: string) => invoke<void>("set_library_folder", { folder }),
  cloudStatus: () => invoke<CloudStatus>("cloud_status"),
  cloudSignIn: () => invoke<CloudStatus>("cloud_sign_in"),
  cloudUsePassphraseOnly: () => invoke<CloudStatus>("cloud_use_passphrase_only"),
  cloudForget: () => invoke<CloudStatus>("cloud_forget"),
  setCloudPassphrase: (passphrase: string) => invoke<CloudStatus>("set_cloud_passphrase", { passphrase }),
  cloudSyncNow: () => invoke<string>("cloud_sync_now"),
  downloadModel: (id: string) => invoke<void>("download_model", { id }),
  deleteModel: (id: string) => invoke<void>("delete_model", { id }),
  polishText: (raw: string) => invoke<Dictation>("polish_text", { raw }),
  toggleDictation: () => invoke<void>("toggle_dictation"),
  requestAccessibility: () => invoke<void>("request_accessibility"),
  checkAccessibility: () => invoke<boolean>("check_accessibility"),
  copy: (text: string) => invoke<void>("copy_text", { text }),
};

export const KINDS: Kind[] = [
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

export const KIND_INFO: Record<Kind, { label: string; blurb: string; example: [string, string] }> = {
  filler: { label: "Filler words", blurb: "um, uh, and “like” when it's just padding", example: ["um so like I think", "so I think"] },
  correction: { label: "Self-corrections", blurb: "false starts, stutters, “no wait, I mean…”", example: ["Tuesday, no wait, Wednesday", "Wednesday"] },
  punctuation: { label: "Punctuation & caps", blurb: "sentence breaks, commas, question marks", example: ["hey are you free", "Hey, are you free?"] },
  grammar: { label: "Grammar", blurb: "agreement, tense, little missing words", example: ["me and him was there", "he and I were there"] },
  slang: { label: "Slang", blurb: "turning casual words into standard ones", example: ["gonna", "going to"] },
  rephrase: { label: "Rewording", blurb: "rephrasing or reordering for clarity", example: ["it's not not good", "it's pretty good"] },
  swearing: { label: "Swearing", blurb: "softening or dropping profanity", example: ["so damn good", "so good"] },
  formatting: { label: "Formatting", blurb: "bullet lists when you run through things, “new line”, “bullet point”", example: ["did this, and this", "- this ⏎ - that"] },
  dictionary: { label: "Your dictionary", blurb: "your own “say this, write that” swaps", example: ["tinker studio", "Tinker Studio"] },
};

export const CLAUDE_MODELS = [
  { value: "claude-opus-5", label: "Claude Opus 5", note: "Best at sounding like you" },
  { value: "claude-sonnet-5", label: "Claude Sonnet 5", note: "Faster, cheaper" },
  { value: "claude-haiku-4-5", label: "Claude Haiku 4.5", note: "Fastest" },
];
