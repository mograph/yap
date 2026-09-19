<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { handsFreeHint, talkKey } from "../lib/util";
  import BrandMark from "./BrandMark.svelte";
  import Icon from "./Icon.svelte";

  const snap = app.snap!;
  const mac = snap.platform === "macos";
  const steps = mac ? ["hi", "ears", "brain", "access", "go"] : ["hi", "ears", "brain", "go"];

  let step = $state(0);
  let key = $state("");
  let error = $state("");
  let downloading = $state(false);
  let axOk = $state(snap.accessibility);

  const model = $derived(snap.models.find((m) => m.id === snap.settings.localModel) ?? snap.models[0]);
  const progress = $derived(app.downloads[model.id]);
  const pct = $derived(progress?.total ? Math.round((progress.received / progress.total) * 100) : 0);
  const current = $derived(steps[step]);

  $effect(() => {
    if (current !== "access") return;
    const t = setInterval(async () => (axOk = await api.checkAccessibility()), 1200);
    return () => clearInterval(t);
  });

  async function download() {
    downloading = true;
    error = "";
    try {
      await api.downloadModel(model.id);
      model.installed = true;
    } catch (e) {
      error = String(e);
    }
    downloading = false;
  }

  async function done() {
    const s = snap.settings;
    if (key.trim()) {
      s.anthropicKey = key.trim();
      s.brain = "claude";
    }
    s.onboarded = true;
    await api.saveSettings($state.snapshot(s));
  }

  const next = () => (step = Math.min(step + 1, steps.length - 1));
</script>

<div class="backdrop" transition:fade={{ duration: 180 }}>
  <div class="sheet card" in:fly={{ y: 16, duration: 280 }}>
    <div class="dots">
      {#each steps as s, i (s)}<i class:on={i <= step}></i>{/each}
    </div>

    {#key current}
      <div class="step" in:fly={{ x: 16, duration: 220 }}>
        {#if current === "hi"}
          <div class="big-logo"><BrandMark size={64} live /></div>
          <h2>Talk like you. Type like you.</h2>
          <p>Hold a key, say what you'd say, and Yap types it into whatever app you're in. It cleans up the ums, not your personality.</p>
          <div class="actions"><button class="btn accent" onclick={next}>Let's set it up</button></div>
        {:else if current === "ears"}
          <span class="eyebrow">Step 1 · Ears</span>
          <h2>Download the listening model</h2>
          <p>It runs right on this {mac ? "Mac" : "computer"}. Private, free, works offline. One-time {model.sizeMb} MB download. You can switch to other models in Settings any time.</p>
          <div class="model card">
            <div>
              <b>{model.label}</b>
              <span class="muted small">{model.maker} · {model.languages}</span>
            </div>
            {#if model.installed}
              <span class="ready"><Icon name="check" size={15} /> Ready</span>
            {:else if downloading}
              <span class="muted small">{progress?.stage === "unpacking" ? "Unpacking…" : `${pct}%`}</span>
            {:else}
              <button class="btn primary sm" onclick={download}><Icon name="download" size={14} />Download</button>
            {/if}
          </div>
          {#if downloading}<div class="bar"><i style:width="{pct}%"></i></div>{/if}
          {#if error}<p class="err">{error}</p>{/if}
          <div class="actions">
            <button class="btn ghost" onclick={next}>{model.installed ? "" : "I'll use a cloud service instead"}</button>
            <button class="btn accent" disabled={!model.installed} onclick={next}>Next</button>
          </div>
        {:else if current === "brain"}
          <span class="eyebrow">Step 2 · Brain</span>
          <h2>Plug in Claude</h2>
          <p>Claude reads your transcript with your voice profile and makes it sound like you typed it. Without a key, Yap still works with simple local cleanup.</p>
          {#if snap.envKey}
            <p class="found"><Icon name="check" size={15} /> Found <code>ANTHROPIC_API_KEY</code> in your environment. You're set.</p>
          {:else}
            <label class="label" for="ob-key">Anthropic API key</label>
            <input id="ob-key" class="field" type="password" placeholder="sk-ant-…" bind:value={key} autocomplete="off" spellcheck="false" />
            <button class="link" onclick={() => openUrl("https://console.anthropic.com/settings/keys")}>Get a key <Icon name="external" size={12} /></button>
          {/if}
          <div class="actions">
            <button class="btn ghost" onclick={next}>Skip for now</button>
            <button class="btn accent" onclick={next}>Next</button>
          </div>
        {:else if current === "access"}
          <span class="eyebrow">Step 3 · Typing</span>
          <h2>Let Yap type for you</h2>
          <p>macOS asks you to allow Yap under Accessibility so it can paste into other apps. Until then, Yap copies your words to the clipboard.</p>
          <div class="model card">
            <div><b>Accessibility</b><span class="muted small">System Settings → Privacy & Security</span></div>
            {#if axOk}
              <span class="ready"><Icon name="check" size={15} /> Allowed</span>
            {:else}
              <button class="btn primary sm" onclick={() => api.requestAccessibility()}><Icon name="shield" size={14} />Allow</button>
            {/if}
          </div>
          <div class="actions"><button class="btn accent" onclick={next}>{axOk ? "Next" : "Do it later"}</button></div>
        {:else}
          <span class="eyebrow">You're in</span>
          <h2>Now just talk</h2>
          <div class="keys">
            <span>Hold</span>
            {#each talkKey(snap.settings, snap.platform) as k, i (i)}<span class="kbd">{k}</span>{/each}
          </div>
          <p>in any app, say something, and let go. {handsFreeHint(snap.settings, snap.platform)}, and Esc cancels. Head to <b>Your voice</b> to teach Yap how you talk.</p>
          <div class="actions"><button class="btn accent" onclick={done}>Start yapping</button></div>
        {/if}
      </div>
    {/key}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }
  .sheet { width: min(520px, calc(100vw - 48px)); padding: 30px 32px 26px; box-shadow: var(--shadow); }
  .dots { display: flex; gap: 6px; margin-bottom: 24px; }
  .dots i { height: 4px; flex: 1; border-radius: 2px; background: var(--line-2); transition: background 0.3s; }
  .dots i.on { background: var(--accent); }
  .step { display: grid; gap: 12px; }
  h2 { font-size: 25px; font-weight: 750; }
  .step > p { color: var(--ink-2); font-size: 14.5px; line-height: 1.6; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 12px; }
  .model { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 14px 16px; background: var(--card-2); }
  .model div { display: grid; gap: 2px; }
  .ready { display: inline-flex; align-items: center; gap: 5px; color: var(--good); font-weight: 600; font-size: 13px; }
  .bar { height: 6px; border-radius: 3px; background: var(--line); overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--accent); transition: width 0.2s; }
  .err { color: var(--bad); font-size: 13px; }
  .found { display: flex; align-items: center; gap: 7px; color: var(--good); font-weight: 560; }
  code { font: 12.5px var(--mono); background: var(--card-2); padding: 1px 5px; border-radius: 5px; }
  .link { justify-self: start; display: inline-flex; align-items: center; gap: 4px; border: 0; background: none; padding: 0; color: var(--accent-2); font-weight: 560; font-size: 13px; }
  .keys { display: flex; align-items: center; gap: 6px; font: 650 17px var(--round); margin: 6px 0 2px; }
  .keys .kbd { height: 34px; min-width: 38px; font-size: 16px; }
  .big-logo { margin-bottom: 4px; }
</style>
