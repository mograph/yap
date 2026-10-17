<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { slide } from "svelte/transition";
  import { api, FEEDBACK_TEMPLATE_IDS, type Note } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import NoteCard from "../components/NoteCard.svelte";
  import Icon from "../components/Icon.svelte";

  const snap = app.snap!;
  let query = $state("");
  let elapsed = $state(0);
  let level = $state(0);
  let pickerOpen = $state(false);

  const phase = $derived(app.noteStatus.phase);
  const recording = $derived(phase === "recording");
  const shown = $derived(
    snap.notes.filter((n) => !query || `${n.title} ${n.summary} ${n.raw}`.toLowerCase().includes(query.toLowerCase())).slice(0, 80),
  );
  const enabledTemplates = $derived(snap.templates.filter((t) => t.enabled));
  let selectedTemplate = $state("auto");
  const AUTO_CARD = { id: "auto", name: "✨ Let Claude pick", description: "Looks at what you said and picks whichever template fits best." };
  const selectedCard = $derived([AUTO_CARD, ...enabledTemplates].find((t) => t.id === selectedTemplate) ?? AUTO_CARD);

  function pick(id: string) {
    selectedTemplate = id;
    pickerOpen = false;
  }

  /** Trello-style label color per card, by category: feedback/critique formats get one
   *  accent, general/meeting formats get another, and the auto-pick card gets its own. */
  function cardAccent(id: string): string {
    if (id === "auto") return "auto";
    return FEEDBACK_TEMPLATE_IDS.has(id) ? "feedback" : "general";
  }

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
    <p>Start a recording and talk through a meeting, a feedback session, or anything else that runs longer than a quick dictation. Yap structures it into a note when you stop.</p>
    {#if phase === "idle" || phase === "error"}
      <div class="mode-row">
        <button class="mode-trigger" class:open={pickerOpen} onclick={() => (pickerOpen = !pickerOpen)} aria-expanded={pickerOpen}>
          <span class="mode-name">{selectedCard.name}</span>
          <span class="chev" class:up={pickerOpen}><Icon name="chevron" size={14} /></span>
        </button>
        <button class="btn ghost sm" onclick={() => (app.view = "settings")}>
          <Icon name="sliders" size={13} />Manage
        </button>
      </div>
      {#if pickerOpen}
        <div class="mode-cards" transition:slide={{ duration: 180 }}>
          <button class="mode-card accent-auto" class:selected={selectedTemplate === "auto"} onclick={() => pick("auto")}>
            <span class="mode-card-label"></span>
            <span class="mode-card-body">
              <span class="mode-card-name">{AUTO_CARD.name}</span>
              <span class="mode-card-desc">{AUTO_CARD.description}</span>
            </span>
          </button>
          {#each enabledTemplates as t (t.id)}
            <button class="mode-card accent-{cardAccent(t.id)}" class:selected={selectedTemplate === t.id} onclick={() => pick(t.id)}>
              <span class="mode-card-label"></span>
              <span class="mode-card-body">
                <span class="mode-card-name">{t.name}</span>
                <span class="mode-card-desc">{t.description || "No description yet."}</span>
              </span>
            </button>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
  <div class="record-wrap">
    {#if phase === "idle" || phase === "error"}
      <button class="record" onclick={() => api.startNote(selectedTemplate)} aria-label="Start a note">
        <span class="dot"></span>
      </button>
      <span class="record-hint">{phase === "error" ? app.noteStatus.message : "Start a note"}</span>
    {:else if phase === "recording"}
      <div class="recording-row">
        <button class="record live" onclick={() => api.stopNote()} aria-label="Stop and structure">
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
      <span class="record-hint">{app.noteStatus.message || "Structuring your note…"}</span>
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
          templates={enabledTemplates}
          {ondelete}
          startOpen={n.id === justCreatedId}
          startPickerOpen={n.id === justCreatedId && FEEDBACK_TEMPLATE_IDS.has(n.templateId)}
        />
      {/each}
    </div>
  {:else if query}
    <p class="empty muted">Nothing matches “{query}”.</p>
  {:else}
    <div class="empty card">
      <h3>No notes yet</h3>
      <p class="muted">Start one above. Unlike push-to-talk dictation, a note keeps recording until you stop it - talk as long as you need, Yap structures it into a title, summary, chapters, decisions and action items when you're done.</p>
    </div>
  {/if}
</section>

<style>
  .hero { display: flex; align-items: flex-start; justify-content: space-between; gap: 30px; margin-bottom: 26px; }
  .mode-row { display: flex; align-items: center; gap: 8px; margin-top: 14px; }
  .mode-trigger {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 10px;
    border-radius: 9px;
    border: 1px solid var(--line-2);
    background: var(--card);
    font-size: 13px;
    font-weight: 560;
    max-width: 260px;
  }
  .mode-trigger:hover, .mode-trigger.open { border-color: var(--accent); }
  .mode-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chev { display: inline-flex; color: var(--ink-3); transition: transform 0.2s; }
  .chev.up { transform: rotate(180deg); }
  .mode-cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: 10px;
    margin-top: 10px;
    max-width: 680px;
  }
  /* Trello-style: a flat white card with a colored label strip across the top, resting
     shadow that lifts (and the card nudges up) on hover - the same "pick me up" affordance
     Trello cards give you, so these read as distinct, clickable tiles rather than a plain
     settings list. */
  .mode-card {
    position: relative;
    text-align: left;
    display: grid;
    gap: 0;
    padding: 0;
    overflow: hidden;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--card);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
    transition: transform 0.15s ease, box-shadow 0.15s ease, border-color 0.15s ease;
  }
  .mode-card:hover { transform: translateY(-2px); box-shadow: 0 8px 16px -6px rgba(0, 0, 0, 0.22); }
  .mode-card.selected { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent-soft); }
  .mode-card-label { display: block; height: 6px; width: 100%; background: var(--ink-3); }
  .mode-card.accent-feedback .mode-card-label { background: #e8793f; }
  .mode-card.accent-general .mode-card-label { background: #4b8bf4; }
  .mode-card.accent-auto .mode-card-label { background: linear-gradient(90deg, #a36cf0, #4b8bf4); }
  .mode-card-body { display: grid; gap: 4px; padding: 10px 12px 12px; }
  .mode-card-name { font-size: 13px; font-weight: 650; }
  .mode-card-desc { font-size: 11.5px; color: var(--ink-3); line-height: 1.4; }

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
