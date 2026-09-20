import type { Dictation } from "./api";

/** "Alt+Space" -> ["⌥", "Space"] on a Mac, ["Alt", "Space"] elsewhere. */
export function keycaps(shortcut: string, platform: string): string[] {
  const mac = platform === "macos";
  return shortcut.split("+").map((part) => {
    const k = part.trim();
    const l = k.toLowerCase();
    if (mac) {
      if (l === "alt" || l === "option") return "⌥";
      if (l === "shift") return "⇧";
      if (l === "ctrl" || l === "control") return "⌃";
      if (["super", "cmd", "command", "meta", "commandorcontrol", "cmdorctrl"].includes(l)) return "⌘";
    } else {
      if (["super", "meta", "cmd", "command"].includes(l)) return "Win";
      if (["commandorcontrol", "cmdorctrl", "control"].includes(l)) return "Ctrl";
    }
    if (l.startsWith("key") && k.length === 4) return k.slice(3);
    if (l.startsWith("digit")) return k.slice(5);
    return k;
  });
}

/** The bare modifier keys you can hold to talk, per platform, as the keycap to show.
 *  Anything else is a key combo. */
const BARE: Record<string, Record<string, string>> = {
  macos: { "option": "⌥", "right-option": "right ⌥", "fn": "fn" },
  windows: { "right-ctrl": "right Ctrl", "right-alt": "right Alt", "ctrl": "Ctrl", "alt": "Alt" },
};

/** The keycap for a bare modifier talk key, or undefined when it's a combo. */
export function bareKey(trigger: string, platform: string): string | undefined {
  return BARE[platform]?.[trigger];
}

/** The key(s) you hold to talk, as keycaps. */
export function talkKey(s: { trigger: string; shortcut: string }, platform: string): string[] {
  const bare = bareKey(s.trigger, platform);
  return bare ? [bare] : keycaps(s.shortcut, platform);
}

/** How hands-free works for the current talk key. A combo can tell a tap from a hold on
 *  its own; a bare modifier needs a double-tap, so it doesn't fire while you use the key. */
export function handsFreeHint(s: { trigger: string }, platform: string) {
  return bareKey(s.trigger, platform) ? "Double-tap for hands-free" : "Tap once for hands-free";
}

/** Is this tidier version a bullet list, or their subjects gathered into paragraphs? */
export const isList = (alt: string) => /^\s*[-*\u2022]\s/m.test(alt);

/** A run of bullets, or a run of ordinary lines. */
export type Block = { list: true; items: string[] } | { list: false; text: string };

/** Splits dictated text into the bullet runs and paragraphs it's made of, so a list renders
 *  as a list instead of lines that happen to start with a dash. */
export function blocks(text: string): Block[] {
  const out: Block[] = [];
  for (const line of text.split("\n")) {
    const bullet = /^\s*[-*\u2022]\s+(.+)$/.exec(line);
    const last = out[out.length - 1];
    if (bullet) {
      if (last && last.list) last.items.push(bullet[1]);
      else out.push({ list: true, items: [bullet[1]] });
    } else if (last && !last.list) {
      last.text += "\n" + line;
    } else {
      out.push({ list: false, text: line });
    }
  }
  return out;
}

export function summarize(list: Dictation[]) {
  const words = list.reduce((n, d) => n + d.words, 0);
  const kept = list.length ? list.reduce((n, d) => n + d.keptPct, 0) / list.length : 0;
  // Typing at ~40 wpm vs. how long you actually talked.
  const savedMin = list.reduce((m, d) => m + Math.max(0, d.words / 40 - d.audioSecs / 60), 0);
  return { words, kept, savedMin, count: list.length };
}

export const compact = (n: number) =>
  new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(n);

export function minutes(min: number) {
  if (min < 1) return `${Math.round(min * 60)}s`;
  if (min < 60) return `${Math.round(min)} min`;
  return `${(min / 60).toFixed(1)} hr`;
}

export function timeAgo(iso: string) {
  const s = (Date.now() - Date.parse(iso)) / 1000;
  if (s < 45) return "just now";
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  const d = new Date(iso);
  if (s < 7 * 86400) return d.toLocaleDateString(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" });
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export function dayKey(d: Date) {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Clean, round axis ticks from 0 to at least `max`. */
export function niceTicks(max: number, count = 4): number[] {
  if (max <= 0) return [0, 1];
  const raw = max / count;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 5, 10].map((s) => s * mag).find((s) => s >= raw) ?? raw;
  const ticks = [];
  for (let v = 0; v <= max + step * 0.001; v += step) ticks.push(v);
  if (ticks[ticks.length - 1] < max) ticks.push(ticks[ticks.length - 1] + step);
  return ticks;
}
