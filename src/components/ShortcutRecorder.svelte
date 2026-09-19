<script lang="ts">
  import { keycaps } from "../lib/util";

  let { value, platform, onchange }: { value: string; platform: string; onchange: (accel: string) => Promise<void> } = $props();
  let recording = $state(false);
  let error = $state("");

  function onkeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      recording = false;
      return;
    }
    if (["Shift", "Control", "Alt", "Meta"].includes(e.key)) return;
    const mods: string[] = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Super");
    let key = e.code;
    if (key.startsWith("Key")) key = key.slice(3);
    else if (key.startsWith("Digit")) key = key.slice(5);
    if (!mods.length && !/^F\d+$/.test(key)) {
      error = platform === "macos" ? "Add a modifier like ⌥, ⌃ or ⇧." : "Add a modifier like Ctrl, Alt or Shift.";
      return;
    }
    recording = false;
    onchange([...mods, key].join("+"))
      .then(() => (error = ""))
      .catch((err) => (error = String(err)));
  }
</script>

<svelte:window {onkeydown} />

<div class="rec">
  <button class="keys" class:recording onclick={() => ((recording = !recording), (error = ""))}>
    {#if recording}
      <span class="listening">Press your new shortcut…</span>
    {:else}
      {#each keycaps(value, platform) as k, i (i)}<span class="kbd">{k}</span>{/each}
    {/if}
  </button>
  <span class="muted small">{recording ? "Esc to keep the current one" : "Click to change"}</span>
</div>
{#if error}<p class="err">{error}</p>{/if}

<style>
  .rec { display: flex; align-items: center; gap: 14px; }
  .keys {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 170px;
    height: 44px;
    padding: 0 12px;
    border-radius: 12px;
    border: 1px dashed var(--line-2);
    background: var(--card-2);
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .keys:hover { border-color: var(--ink-3); }
  .keys.recording { border-style: solid; border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); }
  .keys .kbd { height: 28px; min-width: 30px; font-size: 14px; }
  .listening { color: var(--accent-2); font-weight: 560; }
  .err { margin-top: 8px; color: var(--bad); font-size: 12.5px; }
</style>
