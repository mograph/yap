<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, type Note, type Segment } from "../lib/api";
  import { debounce } from "../lib/state.svelte";
  import { timeAgo } from "../lib/util";
  import Icon from "../components/Icon.svelte";
  import Segmented from "../components/Segmented.svelte";

  let notes = $state<Note[]>([]);
  let selected = $state<string | null>(null);
  let recordingId = $state<string | null>(null);
  let mode = $state<Note["mode"]>("person");
  let busy = $state(false);
  let error = $state("");
  let tab = $state<"notes" | "mine" | "transcript">("notes");
  let levels = $state({ you: 0, them: 0 });
  let now = $state(Date.now());
  let copied = $state(false);
  let confirmDelete = $state(false);
  let transcriptEl = $state<HTMLElement>();

  const note = $derived(notes.find((n) => n.id === selected) ?? null);
  const recording = $derived(!!note && note.id === recordingId);
  const elapsed = $derived(note && recording ? Math.max(0, (now - Date.parse(note.createdAt)) / 1000) : 0);

  function clock(secs: number) {
    const s = Math.floor(secs);
    const hh = Math.floor(s / 3600);
    const mm = Math.floor(s / 60) % 60;
    const ss = String(s % 60).padStart(2, "0");
    return hh ? `${hh}:${String(mm).padStart(2, "0")}:${ss}` : `${mm}:${ss}`;
  }

  function length(secs: number) {
    const m = Math.round(secs / 60);
    return m < 1 ? `${Math.round(secs)} sec` : m < 60 ? `${m} min` : `${Math.floor(m / 60)} hr ${m % 60} min`;
  }

  /** In time order. On a call the mic and the Mac's sound are cut separately, so a line of
   *  yours can arrive after a later line of theirs. */
  const lines = $derived(note ? [...note.segments].sort((a, b) => a.at - b.at) : []);

  const who = (s: Segment) => (s.who === "you" ? "You" : s.who === "them" ? "Them" : "");

  function put(n: Note) {
    const i = notes.findIndex((x) => x.id === n.id);
    if (i === -1) notes.unshift(n);
    else notes[i] = n;
  }

  onMount(() => {
    (async () => {
      notes = await api.notesList();
      recordingId = await api.noteRecording();
      selected = recordingId ?? null;
    })();
    const clockTimer = setInterval(() => (now = Date.now()), 500);
    const offs = [
      listen<{ id: string; segment: Segment }>("yap://note-segment", async (e) => {
        const n = notes.find((x) => x.id === e.payload.id);
        if (!n) return;
        n.segments.push(e.payload.segment);
        // Follow along, unless you've scrolled up to reread something.
        const el = transcriptEl;
        const atBottom = el && el.scrollHeight - el.scrollTop - el.clientHeight < 60;
        await tick();
        if (el && atBottom) el.scrollTop = el.scrollHeight;
      }),
      listen<{ you: number; them: number }>("yap://note-level", (e) => (levels = e.payload)),
    ];
    return () => {
      clearInterval(clockTimer);
      offs.forEach((p) => p.then((off) => off()));
    };
  });

  async function start() {
    busy = true;
    error = "";
    try {
      const n = await api.noteStart(mode);
      put(n);
      selected = n.id;
      recordingId = n.id;
      tab = "mine";
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  async function stop() {
    busy = true;
    error = "";
    try {
      // Save what you typed first, so the notes are laid out around it.
      if (note) await api.noteSave(note.id, note.title, note.myNotes);
      const n = await api.noteStop();
      put(n);
      recordingId = null;
      tab = "notes";
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  const save = debounce(async (id: string, title: string, mine: string) => {
    try {
      put(await api.noteSave(id, title, mine));
    } catch (e) {
      error = String(e);
    }
  }, 600);

  function edited() {
    if (note) save(note.id, note.title, note.myNotes);
  }

  async function copy() {
    if (!note) return;
    await api.copy(note.summary);
    copied = true;
    setTimeout(() => (copied = false), 1400);
  }

  async function remove() {
    if (!note) return;
    if (!confirmDelete) {
      confirmDelete = true;
      setTimeout(() => (confirmDelete = false), 3000);
      return;
    }
    try {
      await api.noteDelete(note.id);
      notes = notes.filter((n) => n.id !== note.id);
      selected = null;
    } catch (e) {
      error = String(e);
    }
    confirmDelete = false;
  }

  /** The laid-out notes as headings, bullets and lines. The title and date are in the header. */
  const sections = $derived.by(() => {
    if (!note?.summary) return [];
    const lines = note.summary.split("\n");
    const first = lines.findIndex((l) => l.startsWith("## "));
    const out: { heading: string; items: { bullet: boolean; text: string; who?: string; at?: string }[] }[] = [];
    for (const line of first === -1 ? [] : lines.slice(first)) {
      if (line.startsWith("## ")) {
        out.push({ heading: line.slice(3), items: [] });
      } else if (line.trim() && out.length) {
        const bullet = line.startsWith("- ");
        const text = bullet ? line.slice(2) : line;
        out[out.length - 1].items.push({ bullet, text });
      }
    }
    // The transcript reads better as a list of who said what, so it gets its own tab.
    return out.filter((s) => s.heading !== "Transcript");
  });
</script>

<div class="page-head">
  <h1>Notes <span class="alpha">Alpha</span></h1>
  <p>Still early: expect rough edges, and keep anything important somewhere else too. Record a meeting, in person or on a call. It's transcribed as you go on this Mac, and when you stop you get your notes, the action items, the decisions, the open questions, and the whole transcript.</p>
</div>

<div class="layout">
  <aside class="list">
    <button class="new" class:active={!selected} onclick={() => (selected = recordingId)} disabled={!!recordingId && selected === recordingId}>
      <Icon name="plus" size={14} />{recordingId ? "Recording…" : "New note"}
    </button>
    {#each notes as n (n.id)}
      <button class="item" class:active={selected === n.id} onclick={() => ((selected = n.id), (tab = n.id === recordingId ? "mine" : "notes"))}>
        <b>{n.title}</b>
        <span class="muted small">
          {#if n.id === recordingId}<span class="live-dot"></span>Recording{:else}{timeAgo(n.createdAt)} · {length(n.durationSecs)}{/if}
          · {n.mode === "call" ? "Call" : "In person"}
        </span>
      </button>
    {:else}
      <p class="muted small empty">Your notes show up here.</p>
    {/each}
  </aside>

  <section class="main">
    {#if error}<p class="error"><Icon name="alert" size={14} />{error}</p>{/if}

    {#if !note}
      <div class="card start">
        <h2>Start a note</h2>
        <Segmented
          label="Where"
          value={mode}
          options={[
            { value: "person", label: "In person" },
            { value: "call", label: "On a call" },
          ]}
          onchange={(v) => (mode = v as Note["mode"])}
        />
        <p class="muted small">
          {#if mode === "person"}
            The mic hears the room, so everyone who's talking ends up in the transcript.
          {:else}
            Your mic is you, and the Mac's own sound is everyone else, so the transcript says who said what. The first time, macOS asks to let Yap record system audio; it's the sound only, never your screen. Headphones give the cleanest split.
          {/if}
        </p>
        <p class="consent small"><Icon name="shield" size={13} />Only record people who know they're being recorded.</p>
        <button class="btn accent record" onclick={start} disabled={busy || !!recordingId}>
          <Icon name="mic" size={16} />{busy ? "Starting…" : "Record"}
        </button>
      </div>
    {:else}
      <header class="note-head">
        <input class="title" bind:value={note.title} oninput={edited} aria-label="Title" spellcheck="false" />
        <div class="meta">
          {#if recording}
            <span class="live"><span class="live-dot"></span>{clock(elapsed)}</span>
            <span class="level">
              {#if note.mode === "call"}You{/if}
              <span class="meter"><i style:transform="scaleX({Math.min(1, levels.you * 8)})"></i></span>
            </span>
            {#if note.mode === "call"}
              <span class="level">Them<span class="meter them"><i style:transform="scaleX({Math.min(1, levels.them * 8)})"></i></span></span>
            {/if}
            <button class="btn sm primary" onclick={stop} disabled={busy}><Icon name="stop" size={13} />{busy ? "Finishing…" : "Stop"}</button>
          {:else}
            <span class="muted small">{new Date(note.createdAt).toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })} · {length(note.durationSecs)} · {note.mode === "call" ? "On a call" : "In person"}</span>
            <button class="btn sm" onclick={copy} disabled={!note.summary}><Icon name={copied ? "check" : "copy"} size={13} />{copied ? "Copied" : "Copy notes"}</button>
            <button class="btn sm ghost" onclick={remove}><Icon name="trash" size={13} />{confirmDelete ? "Delete for good?" : "Delete"}</button>
          {/if}
        </div>
      </header>
      {#if note.warning}<p class="warn small"><Icon name="alert" size={13} />{note.warning}</p>{/if}

      {#if recording}
        <div class="live-grid">
          <div class="card pane">
            <span class="eyebrow">Your notes</span>
            <textarea class="mine" bind:value={note.myNotes} oninput={edited} placeholder="Jot things down as you go. They lead the notes when you stop." spellcheck="true"></textarea>
          </div>
          <div class="card pane">
            <span class="eyebrow">Transcript</span>
            <div class="transcript" bind:this={transcriptEl}>
              {#each lines as s, i (i)}
                <p><span class="who">{who(s) ? `${who(s)} · ` : ""}{clock(s.at)}</span>{s.text}</p>
              {:else}
                <p class="muted small">Listening. Lines appear here every few seconds, after each pause.</p>
              {/each}
            </div>
          </div>
        </div>
      {:else}
        <div class="tabs">
          <Segmented
            label="Show"
            value={tab}
            options={[
              { value: "notes", label: "Notes" },
              { value: "mine", label: "Your notes" },
              { value: "transcript", label: "Transcript" },
            ]}
            onchange={(v) => (tab = v as typeof tab)}
          />
        </div>
        <div class="card body">
          {#if tab === "notes"}
            {#each sections as s (s.heading)}
              <h3>{s.heading}</h3>
              {#if s.items.some((i) => i.bullet)}
                <ul>{#each s.items as item, j (j)}<li>{item.text}</li>{/each}</ul>
              {:else}
                {#each s.items as item, j (j)}<p class="pre">{item.text}</p>{/each}
              {/if}
            {:else}
              <p class="muted">Nothing was transcribed in this one.</p>
            {/each}
          {:else if tab === "mine"}
            <textarea class="mine tall" bind:value={note.myNotes} oninput={edited} placeholder="Add anything you remember. The notes are rebuilt around it."></textarea>
          {:else}
            <div class="transcript full">
              {#each lines as s, i (i)}
                <p><span class="who">{who(s) ? `${who(s)} · ` : ""}{clock(s.at)}</span>{s.text}</p>
              {:else}
                <p class="muted">Nothing was transcribed.</p>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    {/if}
  </section>
</div>

<style>
  .alpha {
    display: inline-block;
    vertical-align: 4px;
    margin-left: 6px;
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-2);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.03em;
  }
  .layout { display: grid; grid-template-columns: 230px minmax(0, 1fr); gap: 18px; align-items: start; }
  .list { display: grid; gap: 4px; position: sticky; top: 0; }
  .list button { text-align: left; border: 0; background: transparent; border-radius: 10px; padding: 9px 11px; color: var(--ink); cursor: pointer; }
  .list button:hover { background: var(--line); }
  .list button.active { background: var(--card); box-shadow: var(--shadow-sm), inset 0 0 0 1px var(--line); }
  .new { display: flex; align-items: center; gap: 7px; font-weight: 600; font-size: 13px; }
  .item { display: grid; gap: 2px; }
  .item b { font-size: 13.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item span { display: flex; align-items: center; gap: 5px; }
  .empty { padding: 8px 11px; }

  .main { display: grid; gap: 14px; min-width: 0; }
  .start { display: grid; gap: 14px; padding: 22px; justify-items: start; }
  .start h2 { margin: 0; font-size: 17px; }
  .start p { margin: 0; max-width: 56ch; line-height: 1.55; }
  .consent { display: flex; align-items: center; gap: 6px; color: var(--ink-2); }
  .record { height: 40px; padding: 0 18px; font-size: 14px; gap: 8px; }

  .note-head { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .title { flex: 1; min-width: 200px; border: 0; background: transparent; font-size: 20px; font-weight: 700; color: var(--ink); padding: 4px 0; outline: none; border-bottom: 1px solid transparent; }
  .title:focus { border-bottom-color: var(--accent); }
  .meta { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .live { display: inline-flex; align-items: center; gap: 7px; font-weight: 650; font-variant-numeric: tabular-nums; color: var(--accent-2); }
  .live-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); animation: pulse 1.2s ease-in-out infinite; flex: none; }
  @keyframes pulse { 50% { opacity: 0.35; } }
  .level { display: inline-flex; align-items: center; gap: 5px; font-size: 11px; font-weight: 600; color: var(--ink-3); }
  .meter { width: 54px; height: 6px; border-radius: 3px; background: var(--line); overflow: hidden; }
  .meter i { display: block; height: 100%; background: var(--accent); transform-origin: left; transition: transform 0.12s; }
  .meter.them i { background: var(--series-1); }

  .warn, .error { display: flex; align-items: flex-start; gap: 7px; margin: 0; padding: 9px 12px; border-radius: 10px; line-height: 1.45; }
  .warn { color: var(--warn); background: var(--warn-soft); }
  .error { color: var(--bad); background: var(--bad-soft); font-size: 13px; }

  .live-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 14px; }
  .pane { display: grid; gap: 8px; padding: 14px 16px; grid-template-rows: auto 1fr; min-height: 220px; }
  .mine { width: 100%; min-height: 180px; resize: vertical; border: 0; background: transparent; color: var(--ink); font: inherit; font-size: 14.5px; line-height: 1.6; outline: none; }
  .mine.tall { min-height: 360px; }
  .transcript { overflow-y: auto; max-height: 460px; display: grid; gap: 10px; align-content: start; user-select: text; -webkit-user-select: text; }
  .transcript.full { max-height: none; }
  .transcript p { margin: 0; font-size: 14px; line-height: 1.55; }
  .who { display: block; font-size: 11px; font-weight: 650; color: var(--ink-3); letter-spacing: 0.02em; margin-bottom: 1px; }

  .tabs { display: flex; }
  .body { padding: 18px 20px; user-select: text; -webkit-user-select: text; }
  .body h3 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.05em; color: var(--ink-3); margin: 18px 0 8px; }
  .body h3:first-child { margin-top: 0; }
  .body ul { margin: 0; padding-left: 18px; display: grid; gap: 6px; }
  .body li { line-height: 1.55; font-size: 14.5px; }
  .body li::marker { color: var(--accent); }
  .pre { margin: 0 0 4px; white-space: pre-wrap; line-height: 1.55; font-size: 14.5px; }
</style>
