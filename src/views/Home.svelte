<script lang="ts">
  import { api, type Dictation } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { compact, minutes, summarize, talkKey } from "../lib/util";
  import DictationCard from "../components/DictationCard.svelte";
  import StatTile from "../components/StatTile.svelte";
  import Icon from "../components/Icon.svelte";

  const snap = app.snap!;
  let query = $state("");

  const week = $derived(summarize(snap.history.filter((d) => Date.now() - Date.parse(d.createdAt) < 7 * 864e5)));
  const shown = $derived(
    snap.history.filter((d) => !query || `${d.text} ${d.raw}`.toLowerCase().includes(query.toLowerCase())).slice(0, 80),
  );
  const greeting = $derived.by(() => {
    const h = new Date().getHours();
    const base = h < 5 ? "Up late" : h < 12 ? "Morning" : h < 18 ? "Hey" : "Evening";
    return snap.profile.name ? `${base}, ${snap.profile.name}.` : `${base}.`;
  });
  const phase = $derived(app.phase.phase);

  function onupdate(d: Dictation) {
    const i = snap.history.findIndex((x) => x.id === d.id);
    if (i >= 0) snap.history[i] = d;
    api.updateDictation(d);
  }

  function ondelete(id: string) {
    const i = snap.history.findIndex((x) => x.id === id);
    if (i >= 0) snap.history.splice(i, 1);
    api.deleteDictation(id);
  }
</script>

<header class="hero">
  <div class="copy">
    <h1>{greeting}</h1>
    <p>
      Hold {#each talkKey(snap.settings, snap.platform) as k, i (i)}<span class="kbd">{k}</span>{/each} anywhere and just talk.
      Yap types it the way you'd type it.
    </p>
    <div class="status-badge">
      {#if snap.settings.autoPaste}
        <span class="badge auto-paste">✓ Auto-paste</span>
      {:else}
        <span class="badge copy-only">📋 Copy only</span>
      {/if}
    </div>
  </div>
  <div class="mic-wrap">
    <button
      class="mic"
      class:live={phase === "listening"}
      class:busy={phase === "thinking"}
      onclick={() => api.toggleDictation()}
      aria-label={phase === "listening" ? "Stop and clean up" : "Start talking"}
    >
      <span class="ring"></span>
      <Icon name="mic" size={24} stroke={2} />
    </button>
    <span class="mic-hint">
      {phase === "listening" ? "Listening… tap to finish" : phase === "thinking" ? app.phase.message : "or try it here"}
    </span>
  </div>
</header>

<div class="tiles">
  <StatTile label="Words this week" value={compact(week.words)} sub="{week.count} dictation{week.count === 1 ? '' : 's'}" />
  <StatTile label="Time saved" value={minutes(week.savedMin)} sub="vs typing at 40 wpm" />
  <StatTile label="Still sounds like you" value={week.count ? `${Math.round(week.kept)}%` : "—"} sub="of your words kept" />
</div>

<section class="section">
  <div class="feed-head">
    <h2>Recent</h2>
    <label class="search field">
      <Icon name="search" size={15} />
      <input placeholder="Search what you've said" bind:value={query} />
    </label>
  </div>

  {#if shown.length}
    <div class="feed">
      {#each shown as d (d.id)}
        <DictationCard item={d} {onupdate} {ondelete} />
      {/each}
    </div>
  {:else if query}
    <p class="empty muted">Nothing matches “{query}”.</p>
  {:else}
    <div class="empty card">
      <div class="wave" aria-hidden="true"><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div>
      <h3>Nothing yet. Say something.</h3>
      <p class="muted">Open any app, hold the shortcut, and talk like you normally would. Everything you dictate shows up here so you can see exactly what Yap changed.</p>
    </div>
  {/if}
</section>

<style>
  .hero { display: flex; align-items: center; justify-content: space-between; gap: 30px; margin-bottom: 26px; }
  .copy h1 { font-size: 36px; font-weight: 750; letter-spacing: -0.03em; }
  .copy p { margin-top: 8px; font-size: 15.5px; color: var(--ink-2); max-width: 44ch; line-height: 1.7; }
  .copy .kbd { margin: 0 2px; vertical-align: 1px; }

  .status-badge { margin-top: 12px; }
  .badge {
    display: inline-block;
    padding: 4px 10px;
    border-radius: 12px;
    font-size: 12px;
    font-weight: 600;
  }
  .badge.auto-paste { background: rgba(40, 167, 69, 0.15); color: #28a745; }
  .badge.copy-only { background: rgba(23, 162, 184, 0.15); color: #17a2b8; }

  .mic-wrap { display: grid; justify-items: center; gap: 10px; flex: none; }
  .mic {
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
  .mic:hover { transform: scale(1.05); }
  .mic:active { transform: scale(0.96); }
  .mic.live { background: var(--accent); color: #fff; }
  .mic.busy { background: var(--ink-2); }
  .ring { position: absolute; inset: -6px; border-radius: 50%; border: 2px solid var(--accent); opacity: 0; }
  .mic.live .ring { animation: ring 1.3s ease-out infinite; }
  @keyframes ring {
    0% { transform: scale(0.9); opacity: 0.8; }
    100% { transform: scale(1.35); opacity: 0; }
  }
  .mic-hint { font-size: 12.5px; color: var(--ink-3); min-height: 18px; }

  .tiles { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }

  .feed-head { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 14px; }
  .search { display: flex; align-items: center; gap: 8px; width: 280px; height: 36px; padding: 0 12px; color: var(--ink-3); }
  .search input { flex: 1; border: 0; outline: none; background: transparent; color: var(--ink); }
  .feed { display: grid; gap: 10px; }

  .empty { padding: 40px 32px; text-align: center; display: grid; justify-items: center; gap: 8px; }
  .empty p { max-width: 52ch; }
  .empty h3 { font-size: 18px; }
  .wave { display: flex; gap: 4px; align-items: center; height: 36px; margin-bottom: 6px; }
  .wave i { width: 5px; border-radius: 3px; background: var(--line-2); animation: idle 1.6s ease-in-out infinite; }
  .wave i:nth-child(odd) { height: 14px; }
  .wave i:nth-child(even) { height: 26px; }
  .wave i:nth-child(2) { animation-delay: 0.1s; }
  .wave i:nth-child(3) { animation-delay: 0.2s; }
  .wave i:nth-child(4) { animation-delay: 0.3s; }
  .wave i:nth-child(5) { animation-delay: 0.4s; }
  .wave i:nth-child(6) { animation-delay: 0.5s; }
  .wave i:nth-child(7) { animation-delay: 0.6s; }
  @keyframes idle {
    0%, 100% { transform: scaleY(0.6); }
    50% { transform: scaleY(1.1); }
  }

  @media (max-width: 760px) {
    .tiles { grid-template-columns: 1fr; }
    .hero { flex-direction: column; align-items: flex-start; }
  }
</style>
