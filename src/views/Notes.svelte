<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { listen } from "@tauri-apps/api/event";
  import { api, type Block, type Note, type Segment } from "../lib/api";
  import { app, debounce } from "../lib/state.svelte";
  import Icon from "../components/Icon.svelte";

  let notes = $state<Note[]>([]);
  let openId = $state<string | null>(null);
  let recordingId = $state<string | null>(null);
  let busy = $state(false);
  let error = $state("");
  let showMine = $state(false);
  let drawer = $state(false);
  let search = $state("");
  let levels = $state({ you: 0, them: 0 });
  let now = $state(Date.now());
  let copied = $state("");
  let confirmDelete = $state(false);
  let editor = $state<HTMLTextAreaElement>();
  let transcriptEl = $state<HTMLElement>();
  let startedAt = $state(Date.now());

  const note = $derived(notes.find((n) => n.id === openId) ?? null);
  const live = $derived(!!note && note.id === recordingId);
  const recordingNote = $derived(notes.find((n) => n.id === recordingId) ?? null);
  const elapsed = $derived(recordingNote ? (recordingNote.durationSecs || 0) + (now - startedAt) / 1000 : 0);
  /** Enhanced notes are on show once there are some, unless you're writing or asked for yours. */
  const enhanced = $derived(!!note && !live && !showMine && note.enhanced.length > 0);
  const twoSides = $derived(!!note?.segments.some((s) => s.who === "them"));
  /** On speakers your mic also hears the call, so the same words turn up as "you" too. The same
   *  check as `without_echo` in notes.rs, so the drawer matches the notes and the copied text. */
  function withoutEcho(segments: Segment[]) {
    const words = (t: string) => new Set(t.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter((w) => w.length > 2));
    const them = segments.filter((s) => s.who === "them").map((s) => ({ at: s.at, words: words(s.text) }));
    return segments.filter((s) => {
      if (s.who !== "you") return true;
      const mine = words(s.text);
      if (mine.size < 4) return true;
      return !them.some((t) => Math.abs(t.at - s.at) < 30 && [...mine].filter((w) => t.words.has(w)).length / mine.size >= 0.6);
    });
  }

  /** In time order: on a call the mic and the Mac's sound are cut separately. */
  const lines = $derived(note ? withoutEcho([...note.segments].sort((a, b) => a.at - b.at)) : []);

  /** Your notes line by line for the copy drawn under the editor, bullets marked out so they can
   *  be drawn as dots. The characters stay the same, so the caret lines up exactly. */
  const drawn = $derived(
    (note?.myNotes ?? "").split("\n").map((line) => {
      const m = /^(\s*)([-*•] )(.*)$/.exec(line);
      return m ? { indent: m[1], marker: m[2], rest: m[3], text: line } : { indent: "", marker: "", rest: "", text: line };
    }),
  );
  const callAudio = $derived(app.snap?.settings.notesCallAudio ?? true);

  function clock(secs: number) {
    const s = Math.max(0, Math.floor(secs));
    const h = Math.floor(s / 3600);
    const m = Math.floor(s / 60) % 60;
    const ss = String(s % 60).padStart(2, "0");
    return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
  }

  function length(secs: number) {
    const m = Math.round(secs / 60);
    return m < 1 ? `${Math.round(secs)} sec` : m < 60 ? `${m} min` : `${Math.floor(m / 60)} hr ${m % 60} min`;
  }

  const time = (iso: string) => new Date(iso).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });

  function day(iso: string) {
    const d = new Date(iso);
    const today = new Date();
    const start = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
    const diff = Math.round((start(today) - start(d)) / 864e5);
    if (diff === 0) return "Today";
    if (diff === 1) return "Yesterday";
    if (diff < 7) return d.toLocaleDateString(undefined, { weekday: "long" });
    return d.toLocaleDateString(undefined, {
      month: "long",
      day: "numeric",
      year: d.getFullYear() === today.getFullYear() ? undefined : "numeric",
    });
  }

  /** Your meetings by day, newest first, narrowed by the search box. */
  const days = $derived.by(() => {
    const q = search.trim().toLowerCase();
    const hit = (n: Note) =>
      !q || [n.title, n.myNotes, ...n.segments.map((s) => s.text)].some((t) => t.toLowerCase().includes(q));
    const out: { label: string; notes: Note[] }[] = [];
    for (const n of [...notes].filter(hit).sort((a, b) => b.createdAt.localeCompare(a.createdAt))) {
      const label = day(n.createdAt);
      const last = out[out.length - 1];
      if (last && last.label === label) last.notes.push(n);
      else out.push({ label, notes: [n] });
    }
    return out;
  });

  const who = (s: { who: string }) => (!twoSides ? "" : s.who === "you" ? "You" : s.who === "them" ? "Them" : "");

  function put(n: Note) {
    const i = notes.findIndex((x) => x.id === n.id);
    if (i === -1) notes.unshift(n);
    else notes[i] = n;
  }

  onMount(() => {
    (async () => {
      notes = await api.notesList();
      recordingId = await api.noteRecording();
      if (recordingId) {
        const n = notes.find((x) => x.id === recordingId);
        // Picked up mid-recording: the clock carries on from when it started.
        if (n) startedAt = Date.parse(n.createdAt);
        openId = recordingId;
        showMine = true;
      }
    })();
    const ticker = setInterval(() => (now = Date.now()), 500);
    const offs = [
      listen<{ id: string; segment: Segment }>("yap://note-segment", async (e) => {
        const n = notes.find((x) => x.id === e.payload.id);
        if (!n) return;
        n.segments.push(e.payload.segment);
        // Follow along in the drawer, unless you've scrolled up to reread something.
        const el = transcriptEl;
        const atBottom = el && el.scrollHeight - el.scrollTop - el.clientHeight < 80;
        await tick();
        if (el && atBottom) el.scrollTop = el.scrollHeight;
      }),
      listen<{ you: number; them: number }>("yap://note-level", (e) => (levels = e.payload)),
    ];
    // ⌘N starts a new note from anywhere in Notes.
    const shortcut = (e: KeyboardEvent) => {
      if (e.metaKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
        if (!recordingId) newNote();
      }
    };
    window.addEventListener("keydown", shortcut);
    return () => {
      clearInterval(ticker);
      window.removeEventListener("keydown", shortcut);
      offs.forEach((p) => p.then((off) => off()));
    };
  });

  /** A blank page, the cursor in it, and listening already, as in Granola. */
  async function newNote() {
    busy = true;
    error = "";
    try {
      const n = await api.noteStart(callAudio ? "call" : "person");
      put(n);
      startedAt = Date.now();
      openId = n.id;
      recordingId = n.id;
      showMine = true;
      drawer = false;
      await tick();
      editor?.focus();
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  async function resume() {
    if (!note) return;
    busy = true;
    error = "";
    try {
      const n = await api.noteStart(callAudio ? "call" : "person", note.id);
      put(n);
      startedAt = Date.now();
      recordingId = n.id;
      showMine = true;
      await tick();
      editor?.focus();
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  async function stop() {
    busy = true;
    error = "";
    try {
      // What you typed goes in first, so the notes are built around it.
      const current = notes.find((x) => x.id === recordingId);
      if (current) await api.noteSave(current.id, current.title, current.myNotes);
      put(await api.noteStop());
      recordingId = null;
      showMine = false;
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  const save = debounce(async (id: string, title: string, mine: string) => {
    try {
      const saved = await api.noteSave(id, title, mine);
      // Keep what's on screen while you type; take only the rebuilt notes from the save.
      const n = notes.find((x) => x.id === id);
      if (n) {
        n.enhanced = saved.enhanced;
        n.summary = saved.summary;
        if (!title.trim()) n.title = saved.title;
      }
    } catch (e) {
      error = String(e);
    }
  }, 600);

  function edited() {
    if (note) save(note.id, note.title, note.myNotes);
    grow();
  }

  function grow() {
    if (!editor) return;
    editor.style.height = "auto";
    editor.style.height = `${editor.scrollHeight}px`;
  }

  /** Bullets carry on when you press Return, stop on an empty one, and Tab nests them. */
  function typing(e: KeyboardEvent) {
    const el = e.currentTarget as HTMLTextAreaElement;
    const { value, selectionStart: at, selectionEnd } = el;
    const lineStart = value.lastIndexOf("\n", at - 1) + 1;
    const lineEnd = value.indexOf("\n", at) === -1 ? value.length : value.indexOf("\n", at);
    const line = value.slice(lineStart, lineEnd);
    const bullet = /^(\s*)([-*•]) (.*)$/.exec(line);
    const replace = (from: number, to: number, text: string, cursor: number) => {
      el.setRangeText(text, from, to, "end");
      el.selectionStart = el.selectionEnd = cursor;
      if (note) note.myNotes = el.value;
      edited();
    };
    if (e.key === "Enter" && !e.shiftKey && bullet && at === selectionEnd) {
      e.preventDefault();
      if (!bullet[3].trim()) {
        // An empty bullet ends the list.
        replace(lineStart, lineEnd, "", lineStart);
      } else {
        const next = `\n${bullet[1]}${bullet[2]} `;
        replace(at, at, next, at + next.length);
      }
    } else if (e.key === "Tab" && bullet) {
      e.preventDefault();
      if (e.shiftKey) {
        if (!bullet[1]) return;
        const cut = Math.min(2, bullet[1].length);
        replace(lineStart, lineStart + cut, "", Math.max(lineStart, at - cut));
      } else {
        replace(lineStart, lineStart, "  ", at + 2);
      }
    }
  }

  async function copy(what: "notes" | "transcript") {
    if (!note) return;
    await api.copy(what === "notes" ? note.summary : await api.noteTranscript(note.id));
    copied = what;
    setTimeout(() => (copied = ""), 1400);
  }

  async function remove() {
    if (!note) return;
    if (!confirmDelete) {
      confirmDelete = true;
      setTimeout(() => (confirmDelete = false), 3000);
      return;
    }
    const id = note.id;
    try {
      await api.noteDelete(id);
      notes = notes.filter((n) => n.id !== id);
      openId = null;
    } catch (e) {
      error = String(e);
    }
    confirmDelete = false;
  }

  async function toggleCallAudio() {
    const s = app.snap?.settings;
    if (!s) return;
    s.notesCallAudio = !s.notesCallAudio;
    await api.saveSettings($state.snapshot(s));
  }

  /** Jump to where a greyed line was said. */
  async function reveal(at: number | null) {
    if (at === null) return;
    drawer = true;
    await tick();
    const row = transcriptEl?.querySelector<HTMLElement>(`[data-at="${Math.floor(at)}"]`);
    row?.scrollIntoView({ block: "center", behavior: "smooth" });
    row?.classList.add("flash");
    setTimeout(() => row?.classList.remove("flash"), 1600);
  }

  function open(n: Note) {
    openId = n.id;
    showMine = n.id === recordingId || n.enhanced.length === 0;
    drawer = false;
    confirmDelete = false;
    error = "";
  }

  $effect(() => {
    // Keep the editor as tall as what's in it whenever it appears.
    if (editor && note) tick().then(grow);
  });

  const barLevels = $derived(
    [0.7, 1, 0.8, 0.55, 0.9].map((k) => Math.min(1, Math.max(levels.you, levels.them) * 9 * k)),
  );
</script>

{#if !note}
  <!-- Home: your meetings, by day. -->
  <div class="home">
    <div class="home-head">
      <h1>Notes <span class="alpha">Alpha</span></h1>
      <button class="btn accent new" onclick={newNote} disabled={busy || !!recordingId}>
        <Icon name="plus" size={15} />New note <kbd>⌘N</kbd>
      </button>
    </div>
    <p class="lede">Start a note and it listens straight away. Type whatever you like while you talk; when you stop, it fills in the rest from what was said. Everything is transcribed on this Mac.</p>

    {#if recordingNote}
      <button class="now" onclick={() => recordingNote && open(recordingNote)}>
        <span class="dot"></span><b>{recordingNote.title}</b><span class="muted">is recording · {clock(elapsed)}</span><span class="go">Open</span>
      </button>
    {/if}

    <div class="tools">
      <input class="field search" placeholder="Search your notes" bind:value={search} spellcheck="false" />
      <button class="switch" class:on={callAudio} onclick={toggleCallAudio} title="On a call, hear the other side through the Mac's own sound">
        <span class="knob"></span>Hear the other side of calls
      </button>
    </div>
    {#if error}<p class="error"><Icon name="alert" size={14} />{error}</p>{/if}

    {#each days as d (d.label)}
      <h2 class="day">{d.label}</h2>
      <div class="rows">
        {#each d.notes as n (n.id)}
          <button class="row" onclick={() => open(n)}>
            <span class="row-title">{n.title}</span>
            {#if n.id === recordingId}
              <span class="rec"><span class="dot"></span>Recording</span>
            {:else}
              <span class="muted">{time(n.createdAt)} · {length(n.durationSecs)}</span>
            {/if}
          </button>
        {/each}
      </div>
    {:else}
      <div class="empty">
        {#if search.trim()}
          <p class="muted">Nothing matches “{search.trim()}”.</p>
        {:else}
          <p><b>No notes yet.</b></p>
          <p class="muted">Press <b>New note</b> when a meeting starts. It works in person and on calls. Only record people who know they're being recorded.</p>
        {/if}
      </div>
    {/each}
  </div>
{:else}
  <!-- A note: the page is yours, the transcript waits in a drawer. -->
  <div class="doc" class:with-drawer={drawer}>
    <div class="doc-top">
      <button class="back" onclick={() => (openId = null)}><Icon name="chevron" size={14} />All notes</button>
      <div class="doc-actions">
        {#if !live && note.summary}
          <button class="btn sm ghost" onclick={() => copy("notes")}><Icon name={copied === "notes" ? "check" : "copy"} size={13} />{copied === "notes" ? "Copied" : "Copy notes"}</button>
        {/if}
        {#if !live}
          <button class="btn sm ghost" onclick={remove}><Icon name="trash" size={13} />{confirmDelete ? "Delete for good?" : "Delete"}</button>
        {/if}
      </div>
    </div>

    <input class="title" bind:value={note.title} oninput={edited} placeholder="Untitled" aria-label="Title" spellcheck="false" />
    <div class="meta">
      {new Date(note.createdAt).toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })} · {time(note.createdAt)}
      {#if live}· <span class="rec"><span class="dot"></span>Transcribing</span>{:else if note.durationSecs}· {length(note.durationSecs)}{/if}
      {#if twoSides}· <span>You and them</span>{/if}
    </div>
    {#if note.warning}<p class="warn"><Icon name="alert" size={13} />{note.warning}</p>{/if}
    {#if error}<p class="error"><Icon name="alert" size={14} />{error}</p>{/if}

    {#if !live && note.enhanced.length}
      <div class="view-switch">
        <button class:on={!showMine} onclick={() => (showMine = false)}><Icon name="sparkle" size={13} />Enhanced</button>
        <button class:on={showMine} onclick={() => (showMine = true)}>My notes</button>
      </div>
    {/if}

    {#if enhanced}
      <article class="enhanced">
        {#each note.enhanced as b, i (i)}
          {@render block(b)}
        {/each}
        <p class="legend"><span class="yours">Black</span> is what you wrote. <span class="added">Grey</span> is what it added from the transcript; click a time to see where it was said.</p>
      </article>
    {:else}
      <div class="editor-wrap">
        <div class="drawn" aria-hidden="true">
          {#each drawn as l, i (i)}
            <div>{#if l.marker}{l.indent}<span class="dash">{l.marker}</span>{l.rest}{:else}{l.text || "\u200b"}{/if}</div>
          {/each}
        </div>
        <textarea
          class="editor"
          bind:this={editor}
          bind:value={note.myNotes}
          oninput={edited}
          onkeydown={typing}
          placeholder={live ? "Start typing…   “- ” makes a bullet, Tab nests it." : "Add anything you remember. The enhanced notes are rebuilt around it."}
          spellcheck="true"
        ></textarea>
      </div>
    {/if}
  </div>

  <!-- The floating pill, as in Granola. -->
  <div class="pill" class:live transition:fly={{ y: 20, duration: 200 }}>
    {#if live}
      <span class="bars" aria-hidden="true">
        {#each barLevels as l, i (i)}<i style:transform="scaleY({0.2 + l * 0.8})"></i>{/each}
      </span>
      <span class="pill-time">{clock(elapsed)}</span>
      <button class="pill-btn" class:on={drawer} onclick={() => (drawer = !drawer)}>Transcript</button>
      <button class="pill-btn stop" onclick={stop} disabled={busy}><Icon name="stop" size={12} />{busy ? "Finishing…" : "Stop"}</button>
    {:else}
      <button class="pill-btn" onclick={resume} disabled={busy || !!recordingId}><Icon name="mic" size={13} />Resume</button>
      <button class="pill-btn" class:on={drawer} onclick={() => (drawer = !drawer)}>Transcript</button>
    {/if}
  </div>

  {#if drawer}
    <aside class="drawer" transition:fly={{ x: 40, duration: 200 }}>
      <div class="drawer-head">
        <b>Transcript</b>
        <div>
          <button class="btn sm ghost" onclick={() => copy("transcript")}><Icon name={copied === "transcript" ? "check" : "copy"} size={13} />{copied === "transcript" ? "Copied" : "Copy"}</button>
          <button class="btn sm ghost" onclick={() => (drawer = false)} aria-label="Close"><Icon name="x" size={14} /></button>
        </div>
      </div>
      <div class="lines" bind:this={transcriptEl}>
        {#each lines as s, i (i)}
          <p data-at={Math.floor(s.at)} class:them={s.who === "them"}>
            <span class="who">{who(s) ? `${who(s)} · ` : ""}{clock(s.at)}</span>{s.text}
          </p>
        {:else}
          <p class="muted" in:fade>{live ? "Listening. Lines appear a few seconds after each pause." : "Nothing was transcribed."}</p>
        {/each}
      </div>
    </aside>
  {/if}
{/if}

{#snippet block(b: Block)}
  {#if b.kind === "heading"}
    <h3 class:added={b.fromTranscript}>{b.text}</h3>
  {:else}
    <p
      class="line"
      class:bullet={b.kind === "bullet"}
      class:added={b.fromTranscript}
      style:padding-left="{b.depth * 22 + (b.kind === 'bullet' ? 18 : 0)}px"
    >
      {b.text}
      {#if b.fromTranscript && b.at !== null}
        <button class="at" onclick={() => reveal(b.at)} title="Show where this was said">{twoSides && b.who === "them" ? "Them · " : ""}{clock(b.at)}</button>
      {/if}
    </p>
  {/if}
{/snippet}

<style>
  .alpha {
    display: inline-block;
    vertical-align: 5px;
    margin-left: 6px;
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-2);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.03em;
  }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); flex: none; animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: 0.35; } }
  .error, .warn { display: flex; align-items: flex-start; gap: 7px; margin: 0 0 12px; padding: 9px 12px; border-radius: 10px; font-size: 13px; line-height: 1.45; }
  .error { color: var(--bad); background: var(--bad-soft); }
  .warn { color: var(--warn); background: var(--warn-soft); }

  /* Home */
  .home { max-width: 720px; }
  .home-head { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  .home-head h1 { margin: 0; font-size: 28px; }
  .new { height: 36px; padding: 0 14px; gap: 7px; font-size: 13.5px; }
  kbd { font: inherit; font-size: 11px; opacity: 0.75; margin-left: 2px; }
  .lede { color: var(--ink-2); line-height: 1.6; margin: 10px 0 20px; max-width: 60ch; }
  .now {
    width: 100%; display: flex; align-items: center; gap: 10px; margin-bottom: 18px; padding: 12px 14px;
    border: 0; border-radius: 12px; background: var(--accent-soft); color: var(--ink); cursor: pointer; text-align: left; font-size: 13.5px;
  }
  .now .go { margin-left: auto; font-weight: 650; color: var(--accent-2); }
  .tools { display: flex; align-items: center; gap: 12px; margin-bottom: 8px; flex-wrap: wrap; }
  .search { flex: 1; min-width: 200px; }
  .switch { display: inline-flex; align-items: center; gap: 8px; border: 0; background: none; color: var(--ink-2); font-size: 12.5px; cursor: pointer; padding: 4px 0; }
  .switch .knob { width: 28px; height: 16px; border-radius: 999px; background: var(--line-2); position: relative; transition: background 0.15s; flex: none; }
  .switch .knob::after { content: ""; position: absolute; top: 2px; left: 2px; width: 12px; height: 12px; border-radius: 50%; background: var(--card); transition: transform 0.15s; box-shadow: var(--shadow-sm); }
  .switch.on .knob { background: var(--accent); }
  .switch.on .knob::after { transform: translateX(12px); }
  .day { font-size: 12px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-3); margin: 22px 0 6px; }
  .rows { display: grid; }
  .row {
    display: flex; align-items: baseline; justify-content: space-between; gap: 16px; padding: 11px 12px; margin: 0 -12px;
    border: 0; border-radius: 10px; background: none; color: var(--ink); cursor: pointer; text-align: left; font-size: 13px;
  }
  .row:hover { background: var(--card); box-shadow: var(--shadow-sm), inset 0 0 0 1px var(--line); }
  .row-title { font-size: 14.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rec { display: inline-flex; align-items: center; gap: 6px; color: var(--accent-2); font-weight: 600; }
  .empty { padding: 30px 0; line-height: 1.6; }
  .empty p { margin: 0 0 4px; }

  /* A note */
  .doc { max-width: 720px; padding-bottom: 120px; transition: margin-right 0.2s; }
  .doc.with-drawer { margin-right: 340px; }
  .doc-top { display: flex; align-items: center; justify-content: space-between; margin-bottom: 18px; }
  .back { display: inline-flex; align-items: center; gap: 4px; border: 0; background: none; color: var(--ink-3); font-size: 13px; cursor: pointer; padding: 4px 0; }
  .back :global(svg) { transform: rotate(90deg); }
  .back:hover { color: var(--ink); }
  .doc-actions { display: flex; gap: 4px; }
  .title { width: 100%; border: 0; background: transparent; color: var(--ink); font-size: 30px; font-weight: 750; letter-spacing: -0.01em; padding: 0; outline: none; }
  .title::placeholder { color: var(--ink-3); }
  .meta { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; color: var(--ink-3); font-size: 13px; margin: 6px 0 20px; }
  .view-switch { display: inline-flex; gap: 2px; padding: 3px; border-radius: 10px; background: var(--card-2); box-shadow: inset 0 0 0 1px var(--line); margin-bottom: 18px; }
  .view-switch button { display: inline-flex; align-items: center; gap: 6px; border: 0; background: none; padding: 5px 12px; border-radius: 7px; font-size: 12.5px; font-weight: 600; color: var(--ink-2); cursor: pointer; }
  .view-switch button.on { background: var(--card); color: var(--ink); box-shadow: var(--shadow-sm); }

  /* You type into a see-through textarea; the copy under it draws the bullets. Both must set text
     exactly alike or the caret drifts from the words. */
  .editor-wrap { position: relative; }
  .editor, .drawn {
    font: inherit; font-size: 16px; line-height: 1.75; letter-spacing: normal;
    white-space: pre-wrap; overflow-wrap: break-word; word-break: normal; tab-size: 2;
    margin: 0; padding: 0; border: 0;
  }
  .drawn { position: absolute; inset: 0; color: var(--ink); pointer-events: none; }
  .dash { position: relative; color: transparent; }
  .dash::before {
    content: ""; position: absolute; left: 0; top: 50%; width: 5px; height: 5px; margin-top: -2px;
    border-radius: 50%; background: var(--ink);
  }
  .editor {
    position: relative; display: block; width: 100%; min-height: 55vh; resize: none; overflow: hidden;
    background: transparent; outline: none; color: transparent; caret-color: var(--ink);
  }
  .editor::placeholder { color: var(--ink-3); }
  .editor::selection { background: var(--accent-soft); }

  .enhanced { user-select: text; -webkit-user-select: text; font-size: 15.5px; line-height: 1.65; }
  .enhanced h3 { font-size: 18px; margin: 26px 0 8px; color: var(--ink); }
  .enhanced h3:first-child { margin-top: 0; }
  .enhanced h3.added { color: var(--ink-2); }
  .line { position: relative; margin: 0 0 6px; color: var(--ink); }
  .line.bullet::before { content: ""; position: absolute; top: 0.72em; width: 5px; height: 5px; border-radius: 50%; background: currentColor; margin-left: -14px; }
  .line.added { color: var(--ink-3); }
  .at {
    display: inline-block; margin-left: 6px; padding: 0 6px; border: 0; border-radius: 6px; background: var(--card-2); color: var(--ink-3);
    font-size: 11px; font-weight: 600; font-variant-numeric: tabular-nums; cursor: pointer; vertical-align: 1px;
  }
  .at:hover { background: var(--accent-soft); color: var(--accent-2); }
  .legend { margin-top: 30px; font-size: 12px; color: var(--ink-3); }
  .legend .yours { color: var(--ink); font-weight: 600; }
  .legend .added { color: var(--ink-3); font-weight: 600; }

  /* The pill */
  .pill {
    position: fixed; bottom: 22px; left: calc(50% + 120px); transform: translateX(-50%); z-index: 20;
    display: flex; align-items: center; gap: 6px; padding: 6px; border-radius: 999px;
    background: var(--card); box-shadow: var(--shadow), inset 0 0 0 1px var(--line);
  }
  .pill.live { padding-left: 14px; }
  .bars { display: inline-flex; align-items: center; gap: 2px; height: 16px; }
  .bars i { width: 3px; height: 16px; border-radius: 2px; background: var(--accent); transform-origin: center; transition: transform 0.12s; }
  .pill-time { font-size: 13px; font-weight: 650; font-variant-numeric: tabular-nums; margin: 0 6px 0 4px; min-width: 38px; }
  .pill-btn { display: inline-flex; align-items: center; gap: 6px; border: 0; border-radius: 999px; padding: 7px 13px; background: none; color: var(--ink-2); font-size: 12.5px; font-weight: 600; cursor: pointer; }
  .pill-btn:hover, .pill-btn.on { background: var(--card-2); color: var(--ink); }
  .pill-btn.stop { background: var(--ink); color: var(--bg); }
  .pill-btn.stop:hover { opacity: 0.9; }
  .pill-btn:disabled { opacity: 0.5; cursor: default; }

  /* The transcript drawer */
  .drawer {
    position: fixed; top: 0; right: 0; bottom: 0; width: 340px; z-index: 15; display: grid; grid-template-rows: auto 1fr;
    background: var(--card); box-shadow: -1px 0 0 var(--line), -12px 0 32px -18px rgba(0, 0, 0, 0.25);
  }
  .drawer-head { display: flex; align-items: center; justify-content: space-between; padding: 16px 14px 10px 18px; }
  .drawer-head > div { display: flex; gap: 2px; }
  .lines { overflow-y: auto; padding: 4px 18px 110px; display: grid; gap: 12px; align-content: start; user-select: text; -webkit-user-select: text; }
  .lines p { margin: 0; font-size: 13.5px; line-height: 1.55; border-radius: 8px; transition: background 0.4s; }
  .lines p.them { color: var(--ink-2); }
  .lines :global(p.flash) { background: var(--accent-soft); }
  .who { display: block; font-size: 11px; font-weight: 650; color: var(--ink-3); margin-bottom: 1px; }
</style>
