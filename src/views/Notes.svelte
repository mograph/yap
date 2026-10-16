<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { api, type Note } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import NoteCard from "../components/NoteCard.svelte";
  import Icon from "../components/Icon.svelte";

  const snap = app.snap!;
  let query = $state("");
  let elapsed = $state(0);
  let level = $state(0);

  const phase = $derived(app.noteStatus.phase);
  const recording = $derived(phase === "recording");
  const shown = $derived(
    snap.notes.filter((n) => !query || `${n.title} ${n.summary} ${n.raw}`.toLowerCase().includes(query.toLowerCase())).slice(0, 80),
  );
  const claudeEnabled = $derived(snap.settings.brain === "claude");

  let justCreatedId = $state("");

  $effect(() => {
    const off = listen<Note>("yap://note", (e) => {
      justCreatedId = e.payload.id;
    });
    return () => void off.then((f) => f());
  });

  // Ticks `elapsed` while recording; re-runs (and its cleanup clears the previous interval)
  // every time `recording` flips, so stopping and starting again always restarts the clock.
  $effect(() => {
    if (!recording) {
      elapsed = 0;
      return;
    }
    const started = Date.now();
    const tick = setInterval(() => (elapsed = Date.now() - started), 200);
    return () => clearInterval(tick);
  });

  // One subscription for the component's lifetime; the callback itself checks `recording`
  // so level ticks from an ordinary push-to-talk dictation (same broadcast event) are ignored.
  $effect(() => {
    const off = listen<number>("yap://level", (e) => {
      if (recording) level = Math.min(1, Math.sqrt(e.payload) * 2.4);
    });
    return () => void off.then((f) => f());
  });

  function ondelete(id: string) {
    const i = snap.notes.findIndex((n) => n.id === id);
    if (i >= 0) snap.notes.splice(i, 1);
    api.deleteNote(id);
  }

  function minSec(ms: number) {
    const s = Math.floor(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }

</script>

<header class="hero">
  <div class="copy">
    <h1>Notes</h1>
    <p>
      Record first, then choose the best format card for this transcription. {#if claudeEnabled}Yap only uses Claude after you choose a format.{:else}The transcription stays on this device until you choose Claude in Settings.{/if}
    </p>
  </div>
  <div class="record-wrap">
    {#if phase === "idle" || phase === "error"}
      <button class="record" onclick={() => api.startNote("raw")} aria-label="Start a note">
        <span class="dot"></span>
      </button>
      <span class="record-hint">{phase === "error" ? app.noteStatus.message : "Start a note"}</span>
    {:else if phase === "recording"}
      <div class="recording-row">
        <button class="record live" onclick={() => api.stopNote()} aria-label="Stop and save transcription">
          <Icon name="check" size={20} stroke={2.4} />
        </button>
        <button class="cancel-btn" onclick={() => api.cancelNote()} aria-label="Discard"><Icon name="x" size={16} /></button>
      </div>
      <span class="record-hint">
        <span class="level" style:transform="scaleY({0.3 + level * 0.7})"></span>
        {minSec(elapsed)} · tap ✓ to finish, ✕ to discard
      </span>
    {:else}
      <button class="record busy" disabled aria-label="Structuring"></button>
      <span class="record-hint">{app.noteStatus.message || "Saving your transcription…"}</span>
    {/if}
  </div>
</header>

<section class="section">
  <div class="feed-head">
    <h2>All notes</h2>
    <label class="search field">
      <Icon name="search" size={15} />
      <input placeholder="Search your notes" bind:value={query} />
    </label>
  </div>

  {#if shown.length}
    <div class="feed">
      {#each shown as n (n.id)}
        <NoteCard
          item={n}
          templates={snap.templates.filter((t) => t.enabled)}
          {ondelete}
          startOpen={n.id === justCreatedId}
          startPickerOpen={n.id === justCreatedId && !n.templateId}
        />
      {/each}
    </div>
  {:else if query}
    <p class="empty muted">Nothing matches “{query}”.</p>
  {:else}
    <div class="empty card">
      <h3>No notes yet</h3>
      <p class="muted">Start one above. Unlike push-to-talk dictation, a note keeps recording until you stop it. When you finish, choose the format card that best fits what you said.</p>
    </div>
  {/if}
</section>

<style>
  .hero { display: flex; align-items: flex-start; justify-content: space-between; gap: 30px; margin-bottom: 26px; }
  .copy h1 { font-size: 36px; font-weight: 750; letter-spacing: -0.03em; }
  .copy p { margin-top: 8px; font-size: 15.5px; color: var(--ink-2); max-width: 46ch; line-height: 1.7; }

  .record-wrap { display: grid; justify-items: center; gap: 10px; flex: none; }
  .record {
    position: relative;
    display: grid;
    place-items: center;
    width: 76px;
    height: 76px;
    border-radius: 50%;
    border: 0;
    background: var(--ink);
    color: var(--bg);
    box-shadow: 0 10px 26px -10px rgba(0, 0, 0, 0.45);
    transition: transform 0.2s cubic-bezier(0.3, 0.9, 0.4, 1.3), background 0.2s;
  }
  .record:hover:not(:disabled) { transform: scale(1.05); }
  .record:active:not(:disabled) { transform: scale(0.96); }
  .record .dot { width: 20px; height: 20px; border-radius: 50%; background: var(--bad); }
  .record.live { background: var(--accent); color: #fff; }
  .record.busy { background: var(--ink-2); opacity: 0.7; }
  .recording-row { display: flex; align-items: center; gap: 10px; }
  .cancel-btn {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 1px solid var(--line-2);
    background: var(--card);
    color: var(--ink-3);
  }
  .cancel-btn:hover { color: var(--bad); border-color: var(--bad); }
  .record-hint { font-size: 12.5px; color: var(--ink-3); min-height: 18px; display: flex; align-items: center; gap: 6px; }
  .level { display: inline-block; width: 3px; height: 14px; border-radius: 2px; background: var(--accent); transition: transform 0.1s linear; }

  .feed-head { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 14px; }
  .search { display: flex; align-items: center; gap: 8px; width: 280px; height: 36px; padding: 0 12px; color: var(--ink-3); }
  .search input { flex: 1; border: 0; outline: none; background: transparent; color: var(--ink); }
  .feed { display: grid; gap: 10px; }

  .empty { padding: 40px 32px; text-align: center; display: grid; justify-items: center; gap: 8px; }
  .empty p { max-width: 52ch; }
  .empty h3 { font-size: 18px; }

  @media (max-width: 760px) {
    .hero { flex-direction: column; align-items: flex-start; }
  }
</style>
