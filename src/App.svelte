<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import { listen } from "@tauri-apps/api/event";
  import { api, CLAUDE_MODELS, type Dictation, type DownloadEvent, type PhaseEvent } from "./lib/api";
  import { app, refresh, type View } from "./lib/state.svelte";
  import { handsFreeHint, talkKey } from "./lib/util";
  import BrandMark from "./components/BrandMark.svelte";
  import Icon from "./components/Icon.svelte";
  import Onboarding from "./components/Onboarding.svelte";
  import Home from "./views/Home.svelte";
  import Voice from "./views/Voice.svelte";
  import Insights from "./views/Insights.svelte";
  import SettingsView from "./views/Settings.svelte";

  const nav: { id: View; label: string; icon: string }[] = [
    { id: "home", label: "Home", icon: "home" },
    { id: "voice", label: "Your voice", icon: "sliders" },
    { id: "insights", label: "Insights", icon: "chart" },
    { id: "settings", label: "Settings", icon: "gear" },
  ];

  onMount(() => {
    refresh();
    const offs = [
      listen<PhaseEvent>("yap://state", (e) => (app.phase = e.payload)),
      listen<Dictation>("yap://dictation", (e) => {
        app.snap?.history.unshift(e.payload);
      }),
      listen<DownloadEvent>("yap://download", (e) => {
        app.downloads[e.payload.id] = e.payload;
        const model = app.snap?.models.find((m) => m.id === e.payload.id);
        if (model && e.payload.done) model.installed = true;
      }),
      // A synced or imported library changed things: update in place so open views keep working.
      listen("yap://library", async () => {
        const fresh = await api.snapshot();
        if (!app.snap) return;
        Object.assign(app.snap.profile, fresh.profile);
        app.snap.history.splice(0, app.snap.history.length, ...fresh.history);
      }),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  });

  const snap = $derived(app.snap);
  const ears = $derived.by(() => {
    if (!snap) return { ok: false, label: "" };
    const s = snap.settings;
    if (s.sttEngine === "cloud") return { ok: !!s.cloudSttKey, label: s.cloudSttKey ? "Cloud" : "Cloud · needs key" };
    const m = snap.models.find((m) => m.id === s.localModel);
    return { ok: !!m?.installed, label: m?.installed ? m.label : `${m?.label ?? "Model"} · not downloaded` };
  });
  const brain = $derived.by(() => {
    if (!snap) return { ok: false, label: "" };
    const s = snap.settings;
    if (s.brain === "local") return { ok: true, label: "Local rules" };
    const hasKey = !!s.anthropicKey || snap.envKey;
    const name = CLAUDE_MODELS.find((m) => m.value === s.claudeModel)?.label.replace("Claude ", "") ?? s.claudeModel;
    return { ok: hasKey, label: hasKey ? `Claude ${name}` : "Claude · needs key" };
  });
  const listening = $derived(app.phase.phase === "listening");
</script>

{#if snap}
  <!-- Only macOS hides its title bar, so only there does the window need its own drag
       strip and room at the top for the traffic lights. -->
  <div class="shell" class:native-chrome={snap.platform !== "macos"}>
    <div class="drag" data-tauri-drag-region></div>
    <aside class="side">
      <div class="brand">
        <BrandMark size={30} live={listening} />
        <span class="word">yap</span>
      </div>
      <nav>
        {#each nav as n (n.id)}
          <button class:active={app.view === n.id} onclick={() => (app.view = n.id)}>
            <Icon name={n.icon} size={17} />
            {n.label}
          </button>
        {/each}
      </nav>
      <div class="spacer"></div>
      <div class="status">
        <div class="hold">
          <span>Hold</span>
          {#each talkKey(snap.settings, snap.platform) as k, i (i)}<span class="kbd">{k}</span>{/each}
        </div>
        <p>and talk anywhere. {handsFreeHint(snap.settings, snap.platform)}, Esc to cancel.</p>
        <ul>
          <li><span class="dot" class:ok={ears.ok}></span>{ears.label}</li>
          <li><span class="dot" class:ok={brain.ok}></span>{brain.label}</li>
        </ul>
      </div>
    </aside>
    <main>
      {#key app.view}
        <div class="content" in:fly={{ y: 8, duration: 220 }}>
          {#if app.view === "home"}
            <Home />
          {:else if app.view === "voice"}
            <Voice />
          {:else if app.view === "insights"}
            <Insights />
          {:else}
            <SettingsView />
          {/if}
        </div>
      {/key}
    </main>
  </div>
  {#if !snap.settings.onboarded}
    <Onboarding />
  {/if}
{:else}
  <div class="boot">
    <BrandMark size={44} live />
    {#if app.error}<p>{app.error}</p>{/if}
  </div>
{/if}

<style>
  .shell { display: grid; grid-template-columns: 236px 1fr; height: 100vh; }
  .drag { position: fixed; top: 0; left: 0; right: 0; height: 38px; z-index: 5; }
  .shell.native-chrome .drag { display: none; }
  .shell.native-chrome .side { padding-top: 16px; }

  .side {
    background: var(--sidebar);
    border-right: 1px solid var(--line);
    padding: 46px 14px 16px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .brand { display: flex; align-items: center; gap: 9px; padding: 0 10px 22px; }
  .word { font: 800 25px/1 var(--round); letter-spacing: -0.03em; }

  nav { display: flex; flex-direction: column; gap: 2px; }
  nav button {
    display: flex;
    align-items: center;
    gap: 11px;
    height: 38px;
    padding: 0 12px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--ink-2);
    font-weight: 560;
    text-align: left;
    transition: background 0.15s, color 0.15s;
  }
  nav button:hover { background: var(--line); color: var(--ink); }
  nav button.active { background: var(--card); color: var(--ink); box-shadow: var(--shadow-sm), inset 0 0 0 1px var(--line); }
  .spacer { flex: 1; }

  .status {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 14px;
    padding: 14px;
    font-size: 12.5px;
    color: var(--ink-2);
  }
  .hold { display: flex; align-items: center; gap: 5px; font: 650 14px var(--round); color: var(--ink); }
  .hold span:first-child { margin-right: 3px; }
  .status p { margin: 8px 0 12px; line-height: 1.45; }
  .status ul { list-style: none; margin: 0; padding: 12px 0 0; border-top: 1px solid var(--line); display: grid; gap: 6px; }
  .status li { display: flex; align-items: center; gap: 8px; }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--warn); flex: none; }
  .dot.ok { background: var(--good); }

  main { overflow-y: auto; min-width: 0; }
  .content { max-width: 900px; margin: 0 auto; padding: 50px 44px 90px; }

  .boot { height: 100vh; display: grid; place-items: center; align-content: center; gap: 14px; color: var(--ink-2); }

  @media (max-width: 880px) {
    .shell { grid-template-columns: 76px 1fr; }
    .word, .status, nav button { font-size: 0; }
    nav button { justify-content: center; padding: 0; }
    .brand { justify-content: center; padding: 0 0 22px; }
    .content { padding: 44px 26px 80px; }
  }
</style>
