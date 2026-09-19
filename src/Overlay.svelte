<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import type { PhaseEvent, Settings } from "./lib/api";
  import { blocks, isList, talkKey } from "./lib/util";

  const BARS = 26;
  const platform = navigator.userAgent.includes("Mac") ? "macos" : "other";

  let phase = $state<PhaseEvent["phase"]>("idle");
  let message = $state("");
  let text = $state("");
  let words = $state(0);
  let locked = $state(false);
  let visible = $state(false);
  let levels = $state<number[]>(Array(BARS).fill(0));
  let elapsed = $state(0);
  let shortcut = $state<string[]>([]);
  let sounds = true;
  let started = 0;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let audio: AudioContext | null = null;

  const clock = $derived(
    `${Math.floor(elapsed / 60000)}:${String(Math.floor(elapsed / 1000) % 60).padStart(2, "0")}`,
  );
  /** What the tidier version on offer actually is, for the wording. */
  const offer = $derived(isList(message) ? "list" : "group");
  /** The tidier version while choosing, trimmed to fit the pill. */
  const preview = $derived.by(() => {
    const lines = message.split("\n").filter(Boolean);
    const shown = lines.length > 5 ? lines.slice(0, 4) : lines;
    return { blocks: blocks(shown.join("\n")), more: lines.length - shown.length };
  });
  const pasted = $derived(message === "Pasted" || message === "Done");

  /** Soft two-tone blip: rising when listening starts, falling when it stops. */
  function blip(up: boolean) {
    if (!sounds) return;
    try {
      audio ??= new AudioContext();
      const t = audio.currentTime;
      const osc = audio.createOscillator();
      const gain = audio.createGain();
      osc.type = "sine";
      osc.frequency.setValueAtTime(up ? 540 : 760, t);
      osc.frequency.exponentialRampToValueAtTime(up ? 820 : 480, t + 0.12);
      gain.gain.setValueAtTime(0.0001, t);
      gain.gain.exponentialRampToValueAtTime(0.07, t + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.2);
      osc.connect(gain).connect(audio.destination);
      osc.start(t);
      osc.stop(t + 0.22);
    } catch {
      // audio is a nicety
    }
  }

  function show(ms?: number) {
    clearTimeout(hideTimer);
    visible = true;
    if (ms) hideTimer = setTimeout(() => (visible = false), ms);
  }

  function onPhase(p: PhaseEvent) {
    const prev = phase;
    phase = p.phase;
    message = p.message;
    text = p.text ?? "";
    words = p.words;
    locked = p.locked;
    switch (p.phase) {
      case "listening":
        if (prev !== "listening") {
          started = Date.now();
          elapsed = 0;
          levels = Array(BARS).fill(0);
          blip(true);
        }
        show();
        break;
      case "thinking":
        if (prev === "listening") blip(false);
        show();
        break;
      case "choose":
        show();
        break;
      case "done":
        show(pasted ? 3200 : 6000);
        break;
      case "error":
        show(5200);
        break;
      default:
        if (p.message) show(1400);
        else {
          clearTimeout(hideTimer);
          visible = false;
        }
    }
  }

  function applySettings(s: Settings) {
    sounds = s.sounds;
    shortcut = talkKey(s, platform);
  }

  onMount(() => {
    invoke<Settings>("get_settings").then(applySettings).catch(() => {});
    const offs = [
      listen<PhaseEvent>("yap://state", (e) => onPhase(e.payload)),
      listen<number>("yap://level", (e) => {
        levels.push(Math.min(1, Math.sqrt(e.payload) * 2.4));
        levels.shift();
      }),
      listen<Settings>("yap://settings", (e) => applySettings(e.payload)),
    ];
    const tick = setInterval(() => {
      if (phase === "listening") elapsed = Date.now() - started;
    }, 200);
    return () => {
      clearInterval(tick);
      offs.forEach((p) => p.then((off) => off()));
    };
  });
</script>

<div class="stage">
  <div class="pill" class:show={visible} data-phase={phase} role="status" aria-live="polite">
    {#if phase === "listening"}
      <span class="rec"></span>
      <span class="wave" aria-hidden="true">
        {#each levels as l, i (i)}
          <i style:transform="scaleY({0.1 + l * 0.9})"></i>
        {/each}
      </span>
      {#if locked}
        <span class="meta">tap {#each shortcut as k, i (i)}<b>{k}</b>{/each} to finish</span>
      {:else}
        <span class="meta clock">{clock}</span>
      {/if}
    {:else if phase === "thinking"}
      <span class="dots" aria-hidden="true"><i></i><i></i><i></i></span>
      <span class="shimmer">{message}</span>
    {:else if phase === "choose"}
      <div class="ask">
        <span class="q">{offer === "list" ? "Make it a list?" : "Group it by subject?"}</span>
        <div class="preview">
          {#each preview.blocks as block, i (i)}
            {#if block.list}
              <ul>
                {#each block.items as line, j (j)}<li>{line}</li>{/each}
              </ul>
            {:else}
              <div class="lead">{block.text}</div>
            {/if}
          {/each}
          {#if preview.more}<div class="more">…and {preview.more} more</div>{/if}
        </div>
        <div class="keys">
          <span>tap {#each shortcut as k, i (i)}<b>{k}</b>{/each} {offer === "list" ? "for a list" : "to group it"}</span>
          <span><b>esc</b> keep it as said</span>
        </div>
        <div class="timer"><i></i></div>
      </div>
    {:else if phase === "done"}
      <span class="badge ok" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5" /></svg>
      </span>
      <div class="typed">
        <span class="said">{text || message}</span>
        {#if !pasted && message}<span class="meta">{message}</span>{/if}
      </div>
    {:else if phase === "error"}
      <span class="badge err" aria-hidden="true">!</span>
      <span class="msg">{message}</span>
    {:else}
      <span class="msg">{message}</span>
    {/if}
  </div>
</div>

<style>
  :global(html), :global(body) { background: transparent !important; }
  .stage {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding-bottom: 16px;
    font: 540 13px/1.35 system-ui, -apple-system, "Segoe UI", sans-serif;
    -webkit-font-smoothing: antialiased;
    user-select: none;
    -webkit-user-select: none;
  }
  .pill {
    display: flex;
    align-items: center;
    gap: 11px;
    min-height: 46px;
    max-width: 416px;
    padding: 8px 18px;
    border-radius: 999px;
    background: rgba(20, 18, 16, 0.94);
    color: #f6f2ea;
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.09),
      0 10px 28px -8px rgba(0, 0, 0, 0.5);
    opacity: 0;
    transform: translateY(12px) scale(0.92);
    transition:
      opacity 0.18s ease,
      transform 0.32s cubic-bezier(0.2, 0.9, 0.3, 1.25);
  }
  .pill.show { opacity: 1; transform: none; }
  .pill[data-phase="error"], .pill[data-phase="done"] { border-radius: 20px; padding: 10px 18px 10px 12px; }
  .pill[data-phase="choose"] { border-radius: 20px; padding: 14px 18px 12px; }

  .rec {
    flex: none;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #ff4d3a;
    animation: pulse 1.4s ease-out infinite;
  }
  .wave { display: flex; align-items: center; gap: 3px; height: 30px; }
  .wave i {
    display: block;
    width: 3px;
    height: 28px;
    border-radius: 2px;
    background: linear-gradient(180deg, #ffffff, #ffb49c);
    transform-origin: center;
    transition: transform 0.09s linear;
  }
  .meta { color: rgba(246, 242, 234, 0.58); font-size: 12px; white-space: nowrap; }
  .clock { font-variant-numeric: tabular-nums; min-width: 30px; }
  .meta b, .keys b {
    font-weight: 620;
    color: #f6f2ea;
    background: rgba(255, 255, 255, 0.13);
    border-radius: 5px;
    padding: 1px 5px;
    margin: 0 1px;
  }

  .dots { display: flex; gap: 4px; }
  .dots i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #ff8a65;
    animation: bounce 1s ease-in-out infinite;
  }
  .dots i:nth-child(2) { animation-delay: 0.12s; }
  .dots i:nth-child(3) { animation-delay: 0.24s; }
  .shimmer {
    background: linear-gradient(90deg, rgba(246, 242, 234, 0.55) 30%, #ffffff 50%, rgba(246, 242, 234, 0.55) 70%);
    background-size: 220% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: shimmer 1.8s linear infinite;
    white-space: nowrap;
  }

  .ask { display: grid; gap: 8px; width: 368px; }
  .q { font-weight: 650; font-size: 14px; }
  .preview {
    display: grid;
    gap: 2px;
    padding: 8px 10px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.07);
    color: rgba(246, 242, 234, 0.92);
  }
  .preview div, .preview li { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .preview .lead { color: rgba(246, 242, 234, 0.66); }
  .preview ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  .preview li { position: relative; padding-left: 14px; }
  .preview li::before {
    content: "";
    position: absolute;
    left: 3px;
    top: 0.62em;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: #ff8a65;
  }
  .preview .more { color: rgba(246, 242, 234, 0.55); }
  .keys { display: flex; justify-content: space-between; color: rgba(246, 242, 234, 0.6); font-size: 11.5px; }
  .timer { height: 3px; border-radius: 2px; background: rgba(255, 255, 255, 0.1); overflow: hidden; }
  .timer i { display: block; height: 100%; background: #ff8a65; animation: countdown 5s linear forwards; }

  .typed { display: grid; gap: 2px; min-width: 0; }
  .said {
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    white-space: pre-line;
    color: #f6f2ea;
  }

  .badge {
    flex: none;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    font-weight: 800;
    align-self: flex-start;
    margin-top: 1px;
  }
  .badge.ok { background: #2fb36a; color: #fff; }
  .badge.err { background: #ff5c45; color: #fff; font-size: 13px; align-self: center; }
  .msg { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }

  @keyframes pulse {
    0% { box-shadow: 0 0 0 0 rgba(255, 77, 58, 0.55); }
    100% { box-shadow: 0 0 0 9px rgba(255, 77, 58, 0); }
  }
  @keyframes bounce {
    0%, 100% { transform: translateY(0); opacity: 0.55; }
    50% { transform: translateY(-4px); opacity: 1; }
  }
  @keyframes shimmer {
    from { background-position: 110% 0; }
    to { background-position: -110% 0; }
  }
  @keyframes countdown {
    from { width: 100%; }
    to { width: 0; }
  }
</style>
