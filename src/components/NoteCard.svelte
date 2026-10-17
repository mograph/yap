<script lang="ts">
  import { slide } from "svelte/transition";
  import { api, FEEDBACK_TEMPLATE_IDS, type Note, type NoteTemplate } from "../lib/api";
  import { timeAgo } from "../lib/util";
  import Icon from "./Icon.svelte";

  let {
    item,
    templates,
    ondelete,
    startOpen = false,
    startPickerOpen = false,
  }: {
    item: Note;
    templates: NoteTemplate[];
    ondelete?: (id: string) => void;
    startOpen?: boolean;
    /** Auto-expand the template picker on mount - used right after a note is created when
     *  Claude's auto-pick detected it as a feedback-style recording, so the cards for
     *  picking exactly which feedback format fits are immediately visible. */
    startPickerOpen?: boolean;
  } = $props();

  // svelte-ignore state_referenced_locally
  let open = $state(startOpen || startPickerOpen);
  // svelte-ignore state_referenced_locally
  let pickerOpen = $state(startPickerOpen);
  // svelte-ignore state_referenced_locally
  let detected = $state(startPickerOpen);
  let copied = $state(false);
  let regenerating = $state(false);
  // svelte-ignore state_referenced_locally
  let regenTemplate = $state(item.templateId || "auto");

  const doneCount = $derived(item.actionItems.filter((a) => a.done).length);
  const minutes = $derived(Math.max(1, Math.round(item.audioSecs / 60)));

  async function regenerate(templateId: string) {
    if (regenerating) return;
    regenTemplate = templateId;
    pickerOpen = false;
    regenerating = true;
    try {
      const updated = await api.restructureNote(item.id, templateId);
      Object.assign(item, updated);
      // Same detection as a freshly created note: if the result landed on a feedback
      // format, keep the cards surfaced (open, not just the trigger) rather than closing
      // and leaving the user to notice and reopen it themselves.
      detected = FEEDBACK_TEMPLATE_IDS.has(updated.templateId);
      if (detected) pickerOpen = true;
    } finally {
      regenerating = false;
    }
  }

  async function copy() {
    try {
      await api.copy(item.summary);
      copied = true;
      setTimeout(() => (copied = false), 1400);
    } catch {
      // The button's own icon reverting is feedback enough; nothing else depends on this.
    }
  }

  function toggle(itemId: string) {
    const a = item.actionItems.find((a) => a.id === itemId);
    if (a) a.done = !a.done;
    api.toggleActionItem(item.id, itemId);
  }
</script>

<article class="card note" class:open>
  <div class="top">
    <div class="text selectable">
      <h3>{item.title || "Untitled note"}</h3>
      <p>{item.summary}</p>
    </div>
    <button class="icon-btn" onclick={copy} title="Copy summary" aria-label="Copy summary">
      <Icon name={copied ? "check" : "copy"} size={16} />
    </button>
  </div>

  <div class="meta">
    {#if item.createdAt}<span>{timeAgo(item.createdAt)}</span>{/if}
    <span class="pill you">{minutes} min</span>
    {#if item.actionItems.length}
      <span class="pill changed">{doneCount}/{item.actionItems.length} done</span>
    {/if}
    <button class="expand" onclick={() => (open = !open)} aria-expanded={open}>
      {open ? "Hide" : "Full note"}
      <span class="chev" class:up={open}><Icon name="chevron" size={14} /></span>
    </button>
  </div>

  {#if item.note}
    <p class="note-flag"><Icon name="alert" size={14} /> {item.note}</p>
  {/if}

  {#if open}
    <div class="detail" transition:slide={{ duration: 220 }}>
      {#each item.chapters as c, i (i)}
        <div class="block">
          <span class="eyebrow">{c.title}</span>
          <p class="body">{c.body}</p>
        </div>
      {/each}

      {#if item.decisions.length}
        <div class="block">
          <span class="eyebrow">Decisions</span>
          <ul class="plain">
            {#each item.decisions as d, i (i)}<li>{d}</li>{/each}
          </ul>
        </div>
      {/if}

      {#if item.actionItems.length}
        <div class="block">
          <span class="eyebrow">Action items</span>
          <ul class="actions">
            {#each item.actionItems as a (a.id)}
              <li>
                <button class="check" class:done={a.done} onclick={() => toggle(a.id)} aria-pressed={a.done}>
                  {#if a.done}<Icon name="check" size={12} />{/if}
                </button>
                <span class:struck={a.done}>{a.text}</span>
                {#if a.owner}<span class="owner">{a.owner}</span>{/if}
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      {#if detected && !regenerating}
        <p class="note-flag detect"><Icon name="sparkle" size={14} /> This sounded like feedback - pick the format that fits best, or leave it as-is.</p>
      {/if}

      <div class="foot">
        <span class="muted small">
          {item.engine}{#if item.sttMs} · heard in {(item.sttMs / 1000).toFixed(1)}s{/if}{#if item.structureMs} · structured in {(item.structureMs / 1000).toFixed(1)}s{/if}
        </span>
        <span class="foot-acts">
          {#if templates.length}
            <button class="btn ghost sm" onclick={() => (pickerOpen = !pickerOpen)} disabled={regenerating}>
              <Icon name="refresh" size={13} />{regenerating ? "Restructuring…" : "Try a different format"}
            </button>
          {/if}
          {#if ondelete}<button class="btn ghost sm" onclick={() => ondelete(item.id)}><Icon name="trash" size={13} />Delete</button>{/if}
        </span>
      </div>

      {#if pickerOpen}
        <div class="mode-cards" transition:slide={{ duration: 180 }}>
          <button class="mode-card accent-auto" class:selected={regenTemplate === "auto"} onclick={() => regenerate("auto")}>
            <span class="mode-card-label"></span>
            <span class="mode-card-body">
              <span class="mode-card-name">✨ Let Claude pick</span>
              <span class="mode-card-desc">Looks at the transcript and picks whichever format fits best.</span>
            </span>
          </button>
          {#each templates as t (t.id)}
            <button class="mode-card accent-{FEEDBACK_TEMPLATE_IDS.has(t.id) ? 'feedback' : 'general'}" class:selected={regenTemplate === t.id} onclick={() => regenerate(t.id)}>
              <span class="mode-card-label"></span>
              <span class="mode-card-body">
                <span class="mode-card-name">{t.name}</span>
                <span class="mode-card-desc">{t.description || "No description yet."}</span>
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</article>

<style>
  .note { padding: 18px 20px 14px; transition: box-shadow 0.2s, border-color 0.2s; }
  .note:hover, .note.open { box-shadow: var(--shadow); }
  .top { display: flex; gap: 14px; align-items: flex-start; }
  .text { flex: 1; min-width: 0; display: grid; gap: 5px; overflow-wrap: anywhere; }
  .text h3 { font-size: 16.5px; font-weight: 650; }
  .text p { margin: 0; font-size: 14px; line-height: 1.6; color: var(--ink-2); white-space: pre-wrap; }
  .icon-btn {
    flex: none;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--ink-3);
    transition: background 0.15s, color 0.15s;
  }
  .icon-btn:hover { background: var(--card-2); border-color: var(--line); color: var(--ink); }

  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-top: 10px; font-size: 12.5px; color: var(--ink-3); }
  .pill { height: 22px; display: inline-flex; align-items: center; padding: 0 9px; border-radius: 999px; font-weight: 600; font-size: 11.5px; }
  .you { background: var(--accent-soft); color: var(--accent-2); }
  .changed { background: var(--card-2); color: var(--ink-2); box-shadow: inset 0 0 0 1px var(--line); }
  .expand { margin-left: auto; display: inline-flex; align-items: center; gap: 4px; border: 0; background: none; color: var(--ink-2); font-weight: 560; font-size: 12.5px; padding: 4px 2px; }
  .expand:hover { color: var(--ink); }
  .chev { display: inline-flex; transition: transform 0.2s; }
  .chev.up { transform: rotate(180deg); }

  .note-flag { display: flex; align-items: center; gap: 7px; margin-top: 10px; font-size: 12.5px; color: var(--warn); background: var(--warn-soft); padding: 7px 10px; border-radius: 9px; }
  .note-flag.detect { color: var(--accent-2); background: var(--accent-soft); margin-top: 0; }

  .detail { margin-top: 14px; padding-top: 14px; border-top: 1px solid var(--line); display: grid; gap: 18px; }
  .block { display: grid; gap: 6px; }
  .eyebrow { font-size: 12px; font-weight: 650; color: var(--ink-2); text-transform: uppercase; letter-spacing: 0.02em; }
  .body { margin: 0; font-size: 14px; line-height: 1.6; white-space: pre-wrap; }
  .plain { list-style: none; margin: 0; padding: 0; display: grid; gap: 5px; font-size: 14px; }
  .plain li { position: relative; padding-left: 17px; }
  .plain li::before { content: ""; position: absolute; left: 3px; top: 0.67em; width: 5px; height: 5px; border-radius: 50%; background: var(--accent); }

  .actions { list-style: none; margin: 0; padding: 0; display: grid; gap: 7px; }
  .actions li { display: flex; align-items: center; gap: 9px; font-size: 14px; }
  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 5px;
    border: 1.5px solid var(--line-2);
    background: var(--card);
    color: #fff;
  }
  .check.done { background: var(--accent); border-color: var(--accent); }
  .struck { text-decoration: line-through; color: var(--ink-3); }
  .owner { margin-left: auto; font-size: 11.5px; color: var(--ink-3); }

  .foot { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px; }
  .foot-acts { display: flex; align-items: center; gap: 6px; }

  .mode-cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 10px;
    margin-top: 10px;
  }
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
</style>
