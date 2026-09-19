<script lang="ts">
  import { slide } from "svelte/transition";
  import { api, KIND_INFO, type Dictation, type Edit } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { blocks, isList, timeAgo } from "../lib/util";
  import DiffText from "./DiffText.svelte";
  import Icon from "./Icon.svelte";

  let {
    item,
    onupdate,
    ondelete,
    startOpen = false,
  }: {
    item: Dictation;
    onupdate: (d: Dictation) => void;
    ondelete?: (id: string) => void;
    startOpen?: boolean;
  } = $props();

  // svelte-ignore state_referenced_locally
  let open = $state(startOpen);
  let copied = $state(false);
  let flash = $state("");

  const body = $derived(blocks(item.text));
  const rows = $derived(item.edits.map((e, i) => ({ e, i })));
  const applied = $derived(rows.filter((r) => r.e.applied));
  const suggested = $derived(rows.filter((r) => !r.e.applied));

  function update(mutate: (d: Dictation) => void) {
    const d = structuredClone($state.snapshot(item)) as Dictation;
    mutate(d);
    onupdate(d);
  }

  const tidy = (s: string) => s.replace(/ {2,}/g, " ").replace(/ ([,.!?])/g, "$1").trim();
  const canUndo = (e: Edit) => !!e.replacement && item.text.includes(e.replacement);
  const canApply = (e: Edit) => !!e.original && item.text.includes(e.original);

  function undo(i: number) {
    update((d) => {
      const e = d.edits[i];
      d.text = tidy(d.text.replace(e.replacement, e.original));
      e.applied = false;
      e.why = "you undid this";
    });
  }

  function apply(i: number) {
    update((d) => {
      const e = d.edits[i];
      d.text = tidy(d.text.replace(e.original, e.replacement));
      e.applied = true;
    });
  }

  function say(msg: string) {
    flash = msg;
    setTimeout(() => (flash = ""), 2600);
  }

  async function keepMine(e: Edit) {
    const profile = app.snap?.profile;
    const word = e.original.replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, "");
    if (!profile || !word) return;
    if (!profile.myWords.some((w) => w.toLowerCase() === word.toLowerCase())) profile.myWords.push(word);
    await api.saveProfile($state.snapshot(profile));
    say(`Got it. Yap won't touch “${word}” again.`);
  }

  /** Kinds where the fix is about a specific word, so "that's my word" makes sense. */
  const WORDY = ["filler", "slang", "swearing", "grammar", "rephrase", "correction"];

  async function justSuggest(e: Edit) {
    const profile = app.snap?.profile;
    if (!profile) return;
    profile.rules[e.kind] = "suggest";
    await api.saveProfile($state.snapshot(profile));
    say(`From now on Yap will only suggest ${KIND_INFO[e.kind].label.toLowerCase()}.`);
  }

  async function alwaysDo(e: Edit) {
    const profile = app.snap?.profile;
    if (!profile) return;
    profile.rules[e.kind] = "do";
    await api.saveProfile($state.snapshot(profile));
    say(`Yap will just fix ${KIND_INFO[e.kind].label.toLowerCase()} from now on.`);
  }

  async function copy() {
    await api.copy(item.text);
    copied = true;
    setTimeout(() => (copied = false), 1400);
  }
</script>

<article class="card dict" class:open>
  <div class="top">
    <div class="text selectable">
      {#each body as block, i (i)}
        {#if block.list}
          <ul class="bullets">
            {#each block.items as line, j (j)}<li>{line}</li>{/each}
          </ul>
        {:else}
          <p>{block.text}</p>
        {/if}
      {/each}
    </div>
    <button class="icon-btn" onclick={copy} title="Copy" aria-label="Copy text">
      <Icon name={copied ? "check" : "copy"} size={16} />
    </button>
  </div>

  <div class="meta">
    {#if item.createdAt}<span>{timeAgo(item.createdAt)}</span>{/if}
    <span class="pill you" title="Share of your own words that made it through">{Math.round(item.keptPct)}% you</span>
    {#if applied.length}<span class="pill changed">{applied.length} change{applied.length === 1 ? "" : "s"}</span>{/if}
    {#if suggested.length}<span class="pill suggested">{suggested.length} suggestion{suggested.length === 1 ? "" : "s"}</span>{/if}
    {#if item.list}
      <button class="pill listify" onclick={() => update((d) => ((d.text = d.list ?? d.text), (d.list = "")))}>
        <Icon name="list" size={12} />{isList(item.list) ? "Make it a list" : "Group by subject"}
      </button>
    {/if}
    <button class="expand" onclick={() => (open = !open)} aria-expanded={open}>
      {open ? "Hide" : "What changed"}
      <span class="chev" class:up={open}><Icon name="chevron" size={14} /></span>
    </button>
  </div>

  {#if item.note}
    <p class="note"><Icon name="alert" size={14} /> {item.note}</p>
  {/if}

  {#if open}
    <div class="detail" transition:slide={{ duration: 220 }}>
      <div class="block">
        <span class="eyebrow">What you said → what got typed</span>
        <DiffText from={item.raw} to={item.text} />
      </div>

      {#if applied.length}
        <div class="block">
          <span class="eyebrow">Changed</span>
          <ul class="edits">
            {#each applied as { e, i } (i)}
              <li>
                <span class="kind">{KIND_INFO[e.kind]?.label ?? e.kind}</span>
                <span class="swap"><s>{e.original}</s><Icon name="chevron" size={12} /><b>{e.replacement || "removed"}</b></span>
                <span class="why">{e.why}</span>
                <span class="acts">
                  {#if canUndo(e)}<button class="btn ghost sm" onclick={() => undo(i)}><Icon name="undo" size={13} />Undo</button>{/if}
                  {#if WORDY.includes(e.kind)}
                    <button class="btn ghost sm" onclick={() => keepMine(e)}><Icon name="heart" size={13} />That's my word</button>
                  {:else if e.kind !== "dictionary"}
                    <button class="btn ghost sm" onclick={() => justSuggest(e)}><Icon name="sliders" size={13} />Just suggest these</button>
                  {/if}
                </span>
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      {#if suggested.length}
        <div class="block">
          <span class="eyebrow">Suggested, left alone</span>
          <ul class="edits">
            {#each suggested as { e, i } (i)}
              <li class="sugg">
                <span class="kind">{KIND_INFO[e.kind]?.label ?? e.kind}</span>
                <span class="swap"><span>{e.original}</span><Icon name="chevron" size={12} /><b>{e.replacement || "remove"}</b></span>
                <span class="why">{e.why}</span>
                <span class="acts">
                  {#if canApply(e)}<button class="btn ghost sm" onclick={() => apply(i)}><Icon name="check" size={13} />Apply</button>{/if}
                  <button class="btn ghost sm" onclick={() => alwaysDo(e)}><Icon name="sparkle" size={13} />Always do this</button>
                </span>
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      <div class="foot">
        <span class="muted small">
          {item.engine}{#if item.sttMs} · heard in {(item.sttMs / 1000).toFixed(1)}s{/if}{#if item.polishMs} · cleaned in {(item.polishMs / 1000).toFixed(1)}s{/if}
        </span>
        {#if ondelete}<button class="btn ghost sm" onclick={() => ondelete(item.id)}><Icon name="trash" size={13} />Delete</button>{/if}
      </div>
    </div>
  {/if}

  {#if flash}<div class="flash" transition:slide={{ duration: 160 }}>{flash}</div>{/if}
</article>

<style>
  .dict { padding: 18px 20px 14px; transition: box-shadow 0.2s, border-color 0.2s; }
  .dict:hover, .dict.open { box-shadow: var(--shadow); }
  .top { display: flex; gap: 14px; align-items: flex-start; }
  .text { flex: 1; min-width: 0; display: grid; gap: 9px; font-size: 15.5px; line-height: 1.6; overflow-wrap: anywhere; }
  .text p { margin: 0; white-space: pre-wrap; }
  .bullets { list-style: none; margin: 0; padding: 0; display: grid; gap: 5px; }
  .bullets li { position: relative; padding-left: 17px; }
  .bullets li::before { content: ""; position: absolute; left: 3px; top: 0.67em; width: 5px; height: 5px; border-radius: 50%; background: var(--accent); }
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
  .suggested { background: var(--warn-soft); color: var(--warn); }
  .listify { gap: 4px; border: 0; cursor: pointer; background: var(--card); color: var(--ink); box-shadow: inset 0 0 0 1px var(--line-2); }
  .listify:hover { box-shadow: inset 0 0 0 1px var(--accent); color: var(--accent-2); }
  .expand { margin-left: auto; display: inline-flex; align-items: center; gap: 4px; border: 0; background: none; color: var(--ink-2); font-weight: 560; font-size: 12.5px; padding: 4px 2px; }
  .expand:hover { color: var(--ink); }
  .chev { display: inline-flex; transition: transform 0.2s; }
  .chev.up { transform: rotate(180deg); }

  .note { display: flex; align-items: center; gap: 7px; margin-top: 10px; font-size: 12.5px; color: var(--warn); background: var(--warn-soft); padding: 7px 10px; border-radius: 9px; }

  .detail { margin-top: 14px; padding-top: 14px; border-top: 1px solid var(--line); display: grid; gap: 18px; }
  .block { display: grid; gap: 8px; }
  .edits { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .edits li {
    display: grid;
    grid-template-columns: 128px minmax(0, 1fr) auto;
    grid-template-areas: "kind swap acts" "kind why acts";
    column-gap: 14px;
    align-items: center;
    padding: 9px 10px 9px 12px;
    border-radius: 11px;
    background: var(--card-2);
  }
  .kind { grid-area: kind; font-size: 12px; font-weight: 650; color: var(--ink-2); }
  .swap { grid-area: swap; display: flex; align-items: center; gap: 6px; flex-wrap: wrap; font-size: 13.5px; }
  .swap :global(svg) { transform: rotate(-90deg); color: var(--ink-3); }
  .swap s { color: var(--bad); text-decoration-thickness: 1.5px; }
  .swap b { color: var(--good); font-weight: 600; }
  .sugg .swap b { color: var(--warn); }
  .why { grid-area: why; font-size: 12px; color: var(--ink-3); }
  .acts { grid-area: acts; display: flex; gap: 2px; }
  .foot { display: flex; justify-content: space-between; align-items: center; }
  .flash { margin-top: 12px; padding: 9px 12px; border-radius: 10px; background: var(--good-soft); color: var(--good); font-weight: 560; font-size: 13px; }
</style>
