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

export async function refresh() {
  try {
    app.snap = await api.snapshot();
  } catch (e) {
    app.error = String(e);
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
