<script lang="ts">
  import { api, KINDS, KIND_INFO, type Dictation, type Rule } from "../lib/api";
  import { app, debounce } from "../lib/state.svelte";
  import ChipsInput from "../components/ChipsInput.svelte";
  import DictationCard from "../components/DictationCard.svelte";
  import Segmented from "../components/Segmented.svelte";
  import Icon from "../components/Icon.svelte";
  import Toggle from "../components/Toggle.svelte";

  const profile = app.snap!.profile;
  /// Local rules do fillers, corrections, punctuation, formatting and your dictionary.
  /// Everything else on this page needs Claude, so say so rather than pretending.
  const localOnly = $derived(app.snap!.settings.brain === "local");
  const CLAUDE_ONLY: string[] = ["grammar", "slang", "rephrase", "swearing"];

  const RULES = [
    { value: "do", label: "Change it" },
    { value: "suggest", label: "Suggest" },
    { value: "leave", label: "Leave it" },
  ];
  const RAW = "um so yeah i'm gonna be like, uh, ten minutes late, traffic is kinda insane rn lol";
  const TONES = [
    { max: 15, name: "Raw", text: "so yeah i'm gonna be like ten minutes late, traffic is kinda insane rn lol" },
    { max: 40, name: "Casual", text: "So yeah, I'm gonna be like ten minutes late, traffic is kinda insane rn lol" },
    { max: 65, name: "Relaxed", text: "So yeah, I'm gonna be about ten minutes late. Traffic is kinda insane right now, lol." },
    { max: 85, name: "Tidy", text: "I'm going to be about ten minutes late. Traffic is pretty bad right now." },
    { max: 100, name: "Polished", text: "I'll be about ten minutes late; traffic is heavy at the moment." },
  ];
  const CASUAL_CAP = 2; // "Relaxed" — casual mode never reads more written than this
  const tier = $derived.by(() => {
    const at = TONES.findIndex((t) => profile.tone <= t.max);
    const i = at === -1 ? TONES.length - 1 : at;
    return TONES[profile.casual ? Math.min(i, CASUAL_CAP) : i];
  });

  let saved = $state(false);
  const save = debounce(async (p: typeof profile) => {
    await api.saveProfile(p);
    saved = true;
    setTimeout(() => (saved = false), 1400);
  }, 450);
  let primed = false;
  $effect(() => {
    const snapshot = $state.snapshot(profile);
    if (!primed) {
      primed = true;
      return;
    }
    save(snapshot);
  });

  let tryText = $state("");
  let result = $state<Dictation | null>(null);
  let trying = $state(false);
  let tryError = $state("");
  async function tryIt() {
    if (!tryText.trim()) return;
    trying = true;
    tryError = "";
    try {
      result = await api.polishText(tryText);
    } catch (e) {
      tryError = String(e);
    }
    trying = false;
  }
</script>

<div class="page-head">
  <div class="title-row">
    <h1>Your voice</h1>
    <span class="saved" class:show={saved}><Icon name="check" size={14} /> Saved</span>
  </div>
  <p>Teach Yap how you actually talk. Everything here goes straight into how your words get cleaned up.</p>
</div>

<section class="card panel">
  <div class="tone-head">
    <div>
      <h2>How polished?</h2>
      <p class="muted small">Slide left to keep it raw, right to tidy it up.</p>
      {#if localOnly}
        <p class="inert"><Icon name="alert" size={13} /> Local rules don't reword, so this slider does nothing right now. Switch the brain to Claude in Settings to use it.</p>
      {/if}
    </div>
    <span class="tier">{tier.name}</span>
  </div>
  <input type="range" min="0" max="100" step="1" bind:value={profile.tone} style:--p="{profile.tone}%" aria-label="Tone" />
  <div class="ends"><span>exactly how I talk</span><span>polished</span></div>
  <div class="example">
    <p class="said"><span class="eyebrow">You say</span>{RAW}</p>
    <p class="typed"><span class="eyebrow">Yap types</span>{tier.text}</p>
  </div>
  <div class="casual">
    <Toggle
      bind:checked={profile.casual}
      label="Casual mode"
      hint={localOnly
        ? "Write it like a Slack message. On local rules this stops anything being turned into bullets or grouped up; the wording half needs Claude."
        : "Write it like a Slack message. Nothing gets turned into bullets or grouped up, and it never reads more written than a chat."}
    />
    {#if profile.casual && profile.tone > TONES[CASUAL_CAP].max}
      <p class="capped"><Icon name="alert" size={13} /> Casual mode caps this at {TONES[CASUAL_CAP].name.toLowerCase()} — slide down or turn it off to go more polished.</p>
    {/if}
  </div>
</section>

<section class="section">
  <h2>What Yap can change</h2>
  <p class="sub">For each kind of edit, pick whether Yap just does it, only suggests it (you'll see it under “What changed”), or leaves it alone entirely.</p>
  <div class="card rules">
    {#each KINDS.filter((k) => k !== "dictionary") as k (k)}
      <div class="rule">
        <div class="rule-text">
          <b>{KIND_INFO[k].label}{#if localOnly && CLAUDE_ONLY.includes(k)}<span class="needs">needs Claude</span>{/if}</b>
          <span class="muted small">{KIND_INFO[k].blurb}</span>
        </div>
        <div class="rule-ex small"><s>{KIND_INFO[k].example[0]}</s><Icon name="chevron" size={11} /><span>{KIND_INFO[k].example[1]}</span></div>
        <Segmented size="sm" label={KIND_INFO[k].label} value={profile.rules[k] ?? "do"} options={RULES} onchange={(v) => (profile.rules[k] = v as Rule)} />
      </div>
    {/each}
  </div>
</section>

<section class="section">
  <h2>Words that are mine</h2>
  <p class="sub">Slang, catchphrases, spellings. Yap never “fixes” these, no matter what the rules say.</p>
  <ChipsInput bind:items={profile.myWords} placeholder="gonna, lowkey, y'all…" />
</section>

<section class="section">
  <h2>Say this, write that</h2>
  <p class="sub">Names, brands, and shortcuts. These also help Whisper hear them right in the first place.</p>
  <div class="card dict">
    {#each profile.dictionary as entry, i (i)}
      <div class="dict-row">
        <input class="field" placeholder="when I say…" bind:value={entry.say} />
        <Icon name="chevron" size={14} />
        <input class="field" placeholder="write…" bind:value={entry.write} />
        <button class="btn ghost sm" aria-label="Remove" onclick={() => profile.dictionary.splice(i, 1)}><Icon name="x" size={14} /></button>
      </div>
    {/each}
    <button class="btn sm add" onclick={() => profile.dictionary.push({ say: "", write: "" })}><Icon name="plus" size={14} />Add a word</button>
  </div>
</section>

<section class="section two">
  <div>
    <h2>About how you talk</h2>
    <p class="sub">Anything Yap should know. “I never capitalize in Slack.” “I say ‘like’ a lot and that's fine.”</p>
    <label class="label" for="name">What should Yap call you?</label>
    <input id="name" class="field" placeholder="Your name" bind:value={profile.name} />
    <textarea class="field notes" placeholder="I talk fast, swear a bit, and keep texts short…" bind:value={profile.notes}></textarea>
  </div>
  <div>
    <h2>Samples of your writing</h2>
    <p class="sub">Paste a few messages you've actually sent. Yap matches the vibe.</p>
    <textarea class="field samples" placeholder={"lol yeah that works for me\nomw, grabbing coffee first\nhonestly i think we should just ship it"} bind:value={profile.samples}></textarea>
  </div>
</section>

<section class="section">
  <h2>Try it</h2>
  <p class="sub">Type something the way you'd say it and see what Yap does with your current settings.</p>
  <div class="card try">
    <textarea class="field" placeholder={"um so basically i was thinking we could, like, push the launch to friday? no wait, thursday"} bind:value={tryText}></textarea>
    <div class="try-foot">
      {#if tryError}<span class="err">{tryError}</span>{:else}<span class="muted small">Uses your Claude key if you've added one</span>{/if}
      <button class="btn accent" disabled={trying || !tryText.trim()} onclick={tryIt}>
        <Icon name="sparkle" size={15} />{trying ? "Thinking…" : "Yap it"}
      </button>
    </div>
  </div>
  {#if result}
    <div class="result">
      <DictationCard item={result} startOpen onupdate={(d) => (result = d)} />
    </div>
  {/if}
</section>

<style>
  .title-row { display: flex; align-items: center; gap: 14px; }
  .saved { display: inline-flex; align-items: center; gap: 5px; color: var(--good); font-size: 13px; font-weight: 600; opacity: 0; transition: opacity 0.25s; }
  .saved.show { opacity: 1; }

  .panel { padding: 22px 24px; }
  .casual { margin-top: 4px; border-top: 1px solid var(--line); }
  .inert { display: flex; align-items: center; gap: 7px; margin: 8px 0 0; font-size: 12.5px; color: var(--ink-3); }
  .needs {
    margin-left: 7px;
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--card-2);
    box-shadow: inset 0 0 0 1px var(--line);
    color: var(--ink-3);
    font-size: 10.5px;
    font-weight: 600;
    vertical-align: 1px;
  }
  .capped {
    display: flex;
    align-items: center;
    gap: 7px;
    margin: 0 0 12px;
    font-size: 12.5px;
    color: var(--warn);
  }
  .tone-head { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 18px; }
  .tone-head h2 { font-size: 17px; }
  .tier { font: 700 13px var(--round); color: var(--accent-2); background: var(--accent-soft); padding: 5px 12px; border-radius: 999px; }
  .ends { display: flex; justify-content: space-between; margin-top: 8px; font-size: 12px; color: var(--ink-3); }
  .example { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-top: 18px; }
  .example p { background: var(--card-2); border-radius: 12px; padding: 12px 14px; font-size: 13.5px; line-height: 1.55; display: grid; gap: 4px; }
  .example .said { color: var(--ink-3); }
  .example .typed { color: var(--ink); background: var(--good-soft); }

  .rules { padding: 4px 18px; }
  .rule { display: grid; grid-template-columns: minmax(170px, 1fr) minmax(0, 1fr) auto; gap: 16px; align-items: center; padding: 14px 0; }
  .rule + .rule { border-top: 1px solid var(--line); }
  .rule-text { display: grid; gap: 1px; }
  .rule-ex { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; color: var(--ink-2); }
  .rule-ex s { color: var(--ink-3); }
  .rule-ex :global(svg) { transform: rotate(-90deg); color: var(--ink-3); }

  .dict { padding: 14px; display: grid; gap: 8px; }
  .dict-row { display: grid; grid-template-columns: 1fr auto 1fr auto; gap: 8px; align-items: center; color: var(--ink-3); }
  .dict-row :global(svg) { transform: rotate(-90deg); }
  .add { justify-self: start; margin-top: 4px; }

  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; }
  .two h2 { font-size: 17px; font-weight: 650; }
  .two .sub { color: var(--ink-2); margin: 4px 0 14px; }
  .notes { margin-top: 10px; min-height: 110px; }
  .samples { min-height: 170px; font-size: 13.5px; }

  .try { padding: 14px; }
  .try textarea { border: 0; background: transparent; box-shadow: none; min-height: 80px; font-size: 15px; }
  .try-foot { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding-top: 10px; border-top: 1px solid var(--line); }
  .err { color: var(--bad); font-size: 12.5px; }
  .result { margin-top: 12px; }

  @media (max-width: 820px) {
    .rule { grid-template-columns: 1fr; gap: 8px; }
    .two, .example { grid-template-columns: 1fr; }
  }
</style>
