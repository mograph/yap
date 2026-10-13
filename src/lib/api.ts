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

export interface ListPreferences {
  bulletStyle: string;
  introText: string;
  itemSpacing: string;
  allowNumbered: boolean;
  indentSpaces: number;
}

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
  /** Customizable list formatting preferences */
  listPreferences: ListPreferences;
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

export interface Chapter {
  title: string;
  body: string;
}

export interface ActionItem {
  id: string;
  text: string;
  owner: string;
  done: boolean;
}

/** A continuous, start/stop recording turned into a structured note - Granola-style
 *  notes mode, alongside push-to-talk dictation. */
export interface Note {
  id: string;
  createdAt: string;
  title: string;
  summary: string;
  chapters: Chapter[];
  decisions: string[];
  actionItems: ActionItem[];
  raw: string;
  audioSecs: number;
  sttMs: number;
  structureMs: number;
  engine: string;
  note: string;
  templateId: string;
}

/** A user-authored Markdown template telling note structuring what mode a note was
 *  recorded in (e.g. "Design feedback") and what to focus on. */
export interface NoteTemplate {
  id: string;
  name: string;
  /** Short, human-facing summary shown on the picker card - distinct from `content`,
   *  the longer instructions actually sent to Claude. */
  description: string;
  content: string;
  /** Whether this is offered in pickers (and eligible for auto-pick) at all. */
  enabled: boolean;
}

/** Built-in templates that are actually about feedback/critique, as opposed to general
 *  meeting formats (1:1, retro) or the plain default. Shared by every surface that offers
 *  the auto-pick template cards, so a note landing on one of these - whether freshly
 *  created or regenerated - consistently gets its picker surfaced for confirmation. */
export const FEEDBACK_TEMPLATE_IDS = new Set([
  "ux-feedback",
  "design-tasks-claude-code",
  "visual-bug-report",
  "like-wish-what-if",
  "user-research-synthesis",
]);

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
  /** Accuracy percentage (0-100) */
  accuracy?: number;
  /** Speed rating: "slow", "medium", "fast" */
  speed?: "slow" | "medium" | "fast";
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
  notes: Note[];
  templates: NoteTemplate[];
  models: ModelInfo[];
  platform: string;
  accessibility: boolean;
  envKey: boolean;
}

export interface NoteStatus {
  phase: "idle" | "recording" | "structuring" | "error";
  message: string;
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
  exportSettings: () => invoke<Settings>("export_settings"),
  importSettings: (settings: Settings) => invoke<void>("import_settings", { settings }),
  startNote: (templateId: string) => invoke<void>("start_note", { templateId }),
  stopNote: () => invoke<void>("stop_note"),
  cancelNote: () => invoke<void>("cancel_note"),
  deleteNote: (id: string) => invoke<void>("delete_note", { id }),
  toggleActionItem: (noteId: string, itemId: string) => invoke<void>("toggle_action_item", { noteId, itemId }),
  saveTemplate: (template: NoteTemplate) => invoke<NoteTemplate>("save_template", { template }),
  deleteTemplate: (id: string) => invoke<void>("delete_template", { id }),
  restructureNote: (id: string, templateId: string) => invoke<Note>("restructure_note", { id, templateId }),
  toggleTemplateEnabled: (id: string) => invoke<void>("toggle_template_enabled", { id }),
  importTemplate: (path: string) => invoke<NoteTemplate>("import_template", { path }),
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
