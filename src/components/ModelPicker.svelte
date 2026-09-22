<script lang="ts">
  import { fly } from "svelte/transition";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { DownloadEvent, ModelInfo } from "../lib/api";
  import Icon from "./Icon.svelte";

  let {
    models,
    value,
    downloads,
    onselect,
    ondownload,
    ondelete,
  }: {
    models: ModelInfo[];
    value: string;
    downloads: Record<string, DownloadEvent>;
    onselect: (id: string) => void;
    ondownload: (id: string) => void;
    ondelete: (id: string) => void;
  } = $props();

  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();

  const selected = $derived(models.find((m) => m.id === value) ?? models[0]);
  const groups = $derived(
    [...new Set(models.map((m) => m.group))].map((name) => ({ name, items: models.filter((m) => m.group === name) })),
  );
  const dl = $derived(downloads[selected.id]);

  const size = (mb: number) => (mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${mb} MB`);
  const pct = (d?: DownloadEvent) => (d?.total ? Math.round((d.received / d.total) * 100) : 0);
  const progress = (d: DownloadEvent) => (d.stage === "unpacking" ? "Unpacking…" : `${pct(d)}%`);

  function pick(id: string) {
    onselect(id);
    open = false;
  }

  function visit(e: Event, url: string) {
    e.stopPropagation();
    openUrl(url);
  }
</script>

<svelte:window
  onclick={(e) => open && root && !root.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => e.key === "Escape" && (open = false)}
/>

<div class="picker" bind:this={root}>
  <div class="anchor">
  <button class="trigger" class:open onclick={() => (open = !open)} aria-haspopup="listbox" aria-expanded={open}>
    <span class="main">
      <span class="name">{selected.label}{#if selected.badge}<span class="badge">{selected.badge}</span>{/if}</span>
      <span class="sub">
        {selected.maker} · {selected.languages}
        <span class="spacer">·</span>
        <span class="size">{size(selected.sizeMb)}</span>
        {#if selected.accuracy !== undefined}
          <span class="spacer">·</span>
          <span class="accuracy">{selected.accuracy}%</span>
        {/if}
        {#if selected.speed}
          <span class="spacer">·</span>
          <span class="speed {selected.speed}">{selected.speed}</span>
        {/if}
      </span>
    </span>
    <span class="state" class:ok={selected.installed}>{selected.installed ? "Ready" : "Not downloaded"}</span>
    <span class="chev" class:up={open}><Icon name="chevron" size={16} /></span>
  </button>

  {#if open}
    <div class="menu" role="listbox" aria-label="Speech model" transition:fly={{ y: -6, duration: 160 }}>
      {#each groups as g (g.name)}
        <div class="group">{g.name}</div>
        {#each g.items as m (m.id)}
          <div
            class="opt"
            class:sel={m.id === value}
            role="option"
            aria-selected={m.id === value}
            tabindex="0"
            onclick={() => pick(m.id)}
            onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), pick(m.id))}
          >
            <span class="check">{#if m.id === value}<Icon name="check" size={15} stroke={2.4} />{/if}</span>
            <span class="main">
              <span class="name">{m.label}{#if m.badge}<span class="badge">{m.badge}</span>{/if}</span>
              <span class="sub">
                {m.maker} · {m.languages}
                <span class="spacer">·</span>
                <span class="size" title="Storage required">{size(m.sizeMb)}</span>
                {#if m.accuracy !== undefined}
                  <span class="spacer">·</span>
                  <span class="accuracy" title="Recognition accuracy">{m.accuracy}% accurate</span>
                {/if}
                {#if m.speed}
                  <span class="spacer">·</span>
                  <span class="speed {m.speed}" title="Processing speed">{m.speed}</span>
                {/if}
              </span>
              <span class="note">{m.note}</span>
            </span>
            <span class="side">
              {#if m.installed}
                <span class="ready"><Icon name="check" size={13} />Ready</span>
              {:else if downloads[m.id]}
                <span class="muted small">{progress(downloads[m.id])}</span>
              {/if}
              <button class="link" onclick={(e) => visit(e, m.link)}>Model page <Icon name="external" size={11} /></button>
            </span>
          </div>
        {/each}
      {/each}
    </div>
  {/if}
  </div>

  <div class="detail">
    <p>{selected.note}{#if selected.usesVocabulary}{" "}Listens for the names and slang in Your voice.{/if}</p>
    <div class="actions">
      <button class="btn ghost sm" onclick={(e) => visit(e, selected.link)}>Model page <Icon name="external" size={12} /></button>
      {#if selected.installed}
        <button class="btn ghost sm" onclick={() => ondelete(selected.id)}><Icon name="trash" size={13} />Remove download</button>
      {:else if dl}
        <span class="bar"><i style:width="{dl.stage === 'unpacking' ? 100 : Math.max(2, pct(dl))}%" class:pulse={dl.stage === "unpacking"}></i></span>
        <span class="muted small">{progress(dl)}</span>
      {:else}
        <button class="btn primary sm" onclick={() => ondownload(selected.id)}><Icon name="download" size={14} />Download · {size(selected.sizeMb)}</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .picker { display: grid; gap: 10px; }
  .anchor { position: relative; }
  .trigger {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 12px 14px;
    border-radius: 12px;
    border: 1px solid var(--line-2);
    background: var(--card);
    text-align: left;
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .trigger:hover { border-color: var(--ink-3); }
  .trigger.open { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); }
  .main { flex: 1; display: grid; gap: 2px; min-width: 0; }
  .name { display: flex; align-items: center; gap: 8px; font-weight: 620; }
  .sub { font-size: 12.5px; color: var(--ink-2); }
  .badge {
    font: 650 10.5px/1 var(--font);
    letter-spacing: 0.02em;
    color: var(--accent-2);
    background: var(--accent-soft);
    padding: 4px 7px;
    border-radius: 999px;
    white-space: nowrap;
  }
  .state { font-size: 12px; font-weight: 600; color: var(--warn); white-space: nowrap; }
  .state.ok { color: var(--good); }
  .chev { display: inline-flex; color: var(--ink-3); transition: transform 0.2s; }
  .chev.up { transform: rotate(180deg); }

  .menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    z-index: 30;
    max-height: 460px;
    overflow-y: auto;
    padding: 6px;
    border-radius: 14px;
    border: 1px solid var(--line-2);
    background: var(--card);
    box-shadow: var(--shadow);
  }
  .group { padding: 10px 10px 4px; font: 600 11px/1 var(--font); letter-spacing: 0.06em; text-transform: uppercase; color: var(--ink-3); }
  .opt {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: start;
    padding: 10px;
    border-radius: 10px;
    cursor: pointer;
    outline: none;
  }
  .opt:hover, .opt:focus-visible { background: var(--card-2); }
  .opt.sel { background: var(--accent-soft); }
  .check { color: var(--accent-2); padding-top: 1px; }
  .note { font-size: 12px; color: var(--ink-3); }
  .side { display: grid; justify-items: end; gap: 6px; }
  .ready { display: inline-flex; align-items: center; gap: 3px; color: var(--good); font-weight: 600; font-size: 12px; }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    border: 0;
    background: none;
    padding: 2px 0;
    color: var(--ink-2);
    font-size: 12px;
    font-weight: 560;
    white-space: nowrap;
  }
  .link:hover { color: var(--accent-2); text-decoration: underline; }

  .detail { display: grid; gap: 8px; padding: 0 2px; }
  .detail p { font-size: 13px; color: var(--ink-2); }
  .actions { display: flex; align-items: center; gap: 8px; }
  .bar { flex: 1; max-width: 260px; height: 6px; border-radius: 3px; background: var(--line); overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--accent); transition: width 0.2s; }
  .bar i.pulse { animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse {
    50% { opacity: 0.45; }
  }

  .spacer { margin: 0 3px; color: var(--line); }
  .size { font-weight: 550; color: var(--ink-2); }
  .accuracy { font-weight: 550; color: #28a745; }
  .speed { font-weight: 550; font-size: 11px; text-transform: capitalize; padding: 2px 6px; border-radius: 4px; }
  .speed.fast { background: #e3f2fd; color: #1976d2; }
  .speed.medium { background: #fff3e0; color: #e65100; }
  .speed.slow { background: #ffebee; color: #c62828; }
</style>
