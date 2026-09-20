import { api, type DownloadEvent, type PhaseEvent, type Snapshot } from "./api";

export type View = "home" | "voice" | "insights" | "settings";

export const app = $state<{
  snap: Snapshot | null;
  view: View;
  phase: PhaseEvent;
  downloads: Record<string, DownloadEvent>;
  error: string;
}>({
  snap: null,
  view: "home",
  phase: { phase: "idle", message: "", words: 0, locked: false, text: "" },
  downloads: {},
  error: "",
});

/** Tauri opens the windows from `tauri.conf.json` before the Rust setup hook has managed the
 *  app state, so a cold start — the first launch after installing, with the virus scanner
 *  reading a brand new binary — can ask for the snapshot a moment too early and get back
 *  "state not managed". Keep asking for a couple of seconds before giving up, so a slow
 *  start shows the app rather than an error the user can only fix by reopening the window. */
export async function refresh() {
  for (let attempt = 0; ; attempt++) {
    try {
      app.snap = await api.snapshot();
      app.error = "";
      return;
    } catch (e) {
      if (attempt >= 20) {
        app.error = String(e);
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
}

/** Calls `fn` once things have been quiet for `ms`. */
export function debounce<A extends unknown[]>(fn: (...args: A) => void, ms: number) {
  let t: ReturnType<typeof setTimeout> | undefined;
  return (...args: A) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...args), ms);
  };
}
