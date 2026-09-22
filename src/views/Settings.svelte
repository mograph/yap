<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, type CloudStatus, CLAUDE_MODELS, type Settings } from "../lib/api";
  import { app, debounce } from "../lib/state.svelte";
  import { handsFreeHint } from "../lib/util";
  import Icon from "../components/Icon.svelte";
  import ModelPicker from "../components/ModelPicker.svelte";
  import Segmented from "../components/Segmented.svelte";
  import ShortcutRecorder from "../components/ShortcutRecorder.svelte";
  import Toggle from "../components/Toggle.svelte";

  const snap = app.snap!;
  const s = snap.settings;
  const mac = snap.platform === "macos";

  const LANGUAGES: [string, string][] = [
    ["auto", "Detect automatically"], ["en", "English"], ["es", "Spanish"], ["fr", "French"], ["de", "German"],
    ["it", "Italian"], ["pt", "Portuguese"], ["nl", "Dutch"], ["pl", "Polish"], ["tr", "Turkish"], ["ru", "Russian"],
    ["ja", "Japanese"], ["ko", "Korean"], ["zh", "Chinese"], ["hi", "Hindi"], ["ar", "Arabic"], ["vi", "Vietnamese"], ["tl", "Tagalog"],
  ];
  const TRIGGERS: Record<string, { value: string; label: string; cap: string; note: string }[]> = {
    macos: [
      { value: "option", label: "Option key", cap: "⌥", note: "Either side. Yap stays out of the way when you use Option in a shortcut or a click." },
      { value: "right-option", label: "Right Option only", cap: "right ⌥", note: "Keeps left Option free for typing symbols and shortcuts." },
      { value: "fn", label: "fn / 🌐 key", cap: "fn", note: "Like Typeless. First set “Press 🌐 key to” → Do Nothing in System Settings → Keyboard." },
      { value: "shortcut", label: "A key combo", cap: "", note: "Pick your own, like ⌥ Space." },
    ],
    windows: [
      { value: "right-ctrl", label: "Right Ctrl", cap: "right Ctrl", note: "The modifier Windows apps leave alone on its own. Left Ctrl stays free for shortcuts." },
      { value: "right-alt", label: "Right Alt", cap: "right Alt", note: "For layouts without AltGr. Where right Alt types accented characters, Yap stays out of the way." },
      { value: "ctrl", label: "Ctrl key", cap: "Ctrl", note: "Either side. Yap stays out of the way when you use Ctrl in a shortcut or a click." },
      { value: "alt", label: "Alt key", cap: "Alt", note: "Either side. On its own, Alt also opens the menu bar in a lot of apps." },
      { value: "shortcut", label: "A key combo", cap: "", note: "Pick your own, like Ctrl Shift Space." },
    ],
  };
  const triggers = TRIGGERS[snap.platform] ?? [];
  const CLOUD_PRESETS = [
    { name: "Groq", url: "https://api.groq.com/openai/v1/audio/transcriptions", model: "whisper-large-v3-turbo" },
    { name: "OpenAI", url: "https://api.openai.com/v1/audio/transcriptions", model: "gpt-4o-transcribe" },
  ];

  let status = $state("");
  let error = $state("");
  let showKey = $state(false);
  let confirmClear = $state(false);
  let axOk = $state(snap.accessibility);
  let modelError = $state("");

  const persist = debounce(async (value: typeof s) => {
    try {
      await api.saveSettings(value);
      error = "";
      status = "Saved";
      setTimeout(() => (status = ""), 1300);
    } catch (e) {
      error = String(e);
    }
  }, 350);
  let primed = false;
  $effect(() => {
    const value = $state.snapshot(s);
    if (!primed) {
      primed = true;
      return;
    }
    persist(value);
  });

  $effect(() => {
    if (!mac || axOk) return;
    api.cloudStatus().then((r) => (cloud = r)).catch(() => {});
    const t = setInterval(async () => (axOk = await api.checkAccessibility()), 1500);
    return () => clearInterval(t);
  });

  async function setShortcut(accel: string) {
    await api.saveSettings({ ...$state.snapshot(s), shortcut: accel });
    s.shortcut = accel;
  }

  const current = $derived(snap.models.find((m) => m.id === s.localModel));

  async function download(id: string) {
    modelError = "";
    app.downloads[id] = { id, received: 0, total: 0, stage: "downloading", done: false };
    try {
      await api.downloadModel(id);
      const m = snap.models.find((m) => m.id === id);
      if (m) m.installed = true;
    } catch (e) {
      modelError = String(e);
    }
    delete app.downloads[id];
  }

  async function remove(id: string) {
    try {
      await api.deleteModel(id);
      const m = snap.models.find((m) => m.id === id);
      if (m) m.installed = false;
    } catch (e) {
      modelError = String(e);
    }
  }

  let libStatus = $state("");
  let settingsStatus = $state("");

  let cloud = $state<CloudStatus>({ registered: false, anonymous: false, email: "", hasPassphrase: false, minPassphrase: 12 });
  let passphrase = $state("");
  let showPass = $state(false);
  let cloudStatus = $state("");
  let cloudError = $state("");
  let cloudBusy = $state(false);
  const canSignIn = $derived(!!s.googleClientId.trim() && !!s.googleClientSecret.trim());

  async function cloudCall(run: () => Promise<unknown>, done: (r: never) => string) {
    cloudBusy = true;
    cloudStatus = "";
    cloudError = "";
    try {
      cloudStatus = done((await run()) as never);
    } catch (e) {
      cloudError = String(e);
    }
    cloudBusy = false;
  }

  const savePassphrase = () =>
    cloudCall(
      () => api.setCloudPassphrase(passphrase),
      (r: CloudStatus) => ((cloud = r), (passphrase = ""), "Passphrase saved. Use the same one everywhere."),
    );
  const signIn = () => cloudCall(api.cloudSignIn, (r: CloudStatus) => ((cloud = r), `Signed in as ${r.email}`));
  const usePassphraseOnly = () =>
    cloudCall(api.cloudUsePassphraseOnly, (r: CloudStatus) => ((cloud = r), "Set up with a passphrase. No account needed."));
  const forget = () =>
    cloudCall(api.cloudForget, (r: CloudStatus) => ((cloud = r), "Forgotten on this computer. Nothing local was touched."));
  const syncNow = () => cloudCall(api.cloudSyncNow, (r: string) => r);

  async function chooseFolder() {
    const dir = await open({ directory: true, title: "Pick a folder that syncs, like Google Drive or iCloud" });
    if (typeof dir === "string") await useFolder(dir);
  }

  async function useFolder(dir: string) {
    try {
      await api.setLibraryFolder(dir);
      s.libraryFolder = dir;
      libStatus = dir ? "Synced just now" : "Stopped syncing";
    } catch (e) {
      libStatus = String(e);
    }
  }

  async function exportLibrary() {
    const path = await save({ defaultPath: "yap-library.json", filters: [{ name: "Yap library", extensions: ["json"] }] });
    if (!path) return;
    try {
      await api.exportLibrary(path);
      libStatus = "Exported";
    } catch (e) {
      libStatus = String(e);
    }
  }

  async function importLibrary() {
    const path = await open({ multiple: false, directory: false, filters: [{ name: "Yap library", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    try {
      const r = await api.importLibrary(path);
      libStatus = `Added ${r.words} word${r.words === 1 ? "" : "s"} and ${r.dictations} dictation${r.dictations === 1 ? "" : "s"}`;
    } catch (e) {
      libStatus = String(e);
    }
  }

  async function clearAll() {
    if (!confirmClear) {
      confirmClear = true;
      setTimeout(() => (confirmClear = false), 3000);
      return;
    }
    await api.clearHistory();
    snap.history.length = 0;
    confirmClear = false;
  }

  async function exportSettings() {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({
      defaultPath: "yap-settings.json",
      filters: [{ name: "Yap Settings", extensions: ["json"] }],
    });
    if (!path) return;
    try {
      const settings = await api.exportSettings();
      const json = JSON.stringify(settings, null, 2);
      await import("@tauri-apps/api/fs").then(m => m.writeTextFile(path, json));
      settingsStatus = "Settings exported to " + path;
      setTimeout(() => (settingsStatus = ""), 3000);
    } catch (e) {
      settingsStatus = "Export failed: " + String(e);
    }
  }

  async function importSettings() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Yap Settings", extensions: ["json"] }],
    });
    if (typeof path !== "string") return;
    try {
      const content = await import("@tauri-apps/api/fs").then(m => m.readTextFile(path));
      const settings = JSON.parse(content);
      await api.importSettings(settings);
      Object.assign(s, settings);
      settingsStatus = "Settings imported successfully";
      setTimeout(() => (settingsStatus = ""), 3000);
    } catch (e) {
      settingsStatus = "Import failed: " + String(e);
    }
  }

  function generateListPreview(prefs: typeof s.listPreferences): string {
    const items = ["First item", "Second item", "Third item"];
    let result = "";
    if (prefs.introText) result += prefs.introText + "\n";

    for (let i = 0; i < items.length; i++) {
      const spacing = prefs.itemSpacing === "double" && i > 0 ? "\n" : "";
      let prefix = "";
      switch (prefs.bulletStyle) {
        case "dash":
          prefix = "-";
          break;
        case "asterisk":
          prefix = "*";
          break;
        case "bullet":
          prefix = "•";
          break;
        case "number":
          prefix = String(i + 1) + ".";
          break;
        default:
          prefix = prefs.bulletStyle;
      }
      const indent = " ".repeat(prefs.indentSpaces);
      result += spacing + indent + prefix + " " + items[i] + "\n";
    }
    return result;
  }
</script>

<div class="page-head">
  <div class="title-row">
    <h1>Settings</h1>
    {#if status}<span class="saved"><Icon name="check" size={14} /> {status}</span>{/if}
  </div>
  {#if error}<p class="err">{error}</p>{/if}
</div>

<section class="card group">
  <h2>Talk key</h2>
  <p class="sub">
    Hold it to talk and let go to finish. {handsFreeHint(s, snap.platform)}, then tap again to finish. Esc cancels.
  </p>
  {#if triggers.length}
    <div class="models">
      {#each triggers as t (t.value)}
        <label class="model" class:selected={s.trigger === t.value}>
          <input type="radio" name="trigger" value={t.value} bind:group={s.trigger} />
          <span class="m-text"><b>{t.label}</b><span class="muted small">{t.note}</span></span>
          {#if t.cap}<span class="kbd">{t.cap}</span>{/if}
        </label>
      {/each}
    </div>
  {/if}
  {#if !triggers.length || s.trigger === "shortcut"}
    <ShortcutRecorder value={s.shortcut} platform={snap.platform} onchange={setShortcut} />
  {/if}
</section>

<section class="card group">
  <div class="group-head">
    <div>
      <h2>Ears</h2>
      <p class="sub">How Yap turns your voice into words.</p>
    </div>
    <Segmented
      label="Transcription engine"
      value={s.sttEngine}
      options={[
        { value: "local", label: "On this device" },
        { value: "cloud", label: "Cloud" },
      ]}
      onchange={(v) => (s.sttEngine = v as "local" | "cloud")}
    />
  </div>

  {#if s.sttEngine === "local"}
    <ModelPicker
      models={snap.models}
      value={s.localModel}
      downloads={app.downloads}
      onselect={(id) => (s.localModel = id)}
      ondownload={download}
      ondelete={remove}
    />
    {#if modelError}<p class="err">{modelError}</p>{/if}
    {#if current && current.languageMode !== "auto"}
      <div class="row-field">
        <label class="label" for="lang">Language</label>
        <select id="lang" class="field" bind:value={s.language}>
          {#each LANGUAGES as [code, name] (code)}<option value={code}>{name}</option>{/each}
        </select>
        <span class="muted small">
          {current.languageMode === "pick"
            ? "This one needs to know which language you speak. Automatic means English."
            : "Leave it on automatic unless it keeps guessing wrong."}
        </span>
      </div>
    {:else}
      <p class="muted small">This model works out which language you're speaking by itself.</p>
    {/if}
  {:else}
    <p class="sub">Any OpenAI-compatible transcription API. Groq is very fast and cheap; handy on older machines.</p>
    <div class="presets">
      {#each CLOUD_PRESETS as p (p.name)}
        <button class="btn sm" class:on={s.cloudSttUrl === p.url} onclick={() => ((s.cloudSttUrl = p.url), (s.cloudSttModel = p.model))}>{p.name}</button>
      {/each}
    </div>
    <div class="grid2">
      <div><label class="label" for="c-url">Endpoint</label><input id="c-url" class="field" bind:value={s.cloudSttUrl} spellcheck="false" /></div>
      <div><label class="label" for="c-model">Model</label><input id="c-model" class="field" bind:value={s.cloudSttModel} spellcheck="false" /></div>
    </div>
    <label class="label" for="c-key">API key</label>
    <input id="c-key" class="field" type="password" bind:value={s.cloudSttKey} autocomplete="off" spellcheck="false" />
  {/if}
</section>

<section class="card group">
  <div class="group-head">
    <div>
      <h2>Brain</h2>
      <p class="sub">What cleans up your words using your voice profile.</p>
    </div>
    <Segmented
      label="Cleanup engine"
      value={s.brain}
      options={[
        { value: "claude", label: "Claude" },
        { value: "local", label: "Local rules only" },
      ]}
      onchange={(v) => (s.brain = v as "claude" | "local")}
    />
  </div>

  {#if s.brain === "claude"}
    <label class="label" for="a-key">Anthropic API key</label>
    <div class="key-row">
      <input id="a-key" class="field" type={showKey ? "text" : "password"} placeholder={snap.envKey ? "Using ANTHROPIC_API_KEY from your environment" : "sk-ant-…"} bind:value={s.anthropicKey} autocomplete="off" spellcheck="false" />
      <button class="btn sm" onclick={() => (showKey = !showKey)}>{showKey ? "Hide" : "Show"}</button>
      <button class="btn ghost sm" onclick={() => openUrl("https://console.anthropic.com/settings/keys")}>Get a key <Icon name="external" size={12} /></button>
    </div>
    <div class="models">
      {#each CLAUDE_MODELS as m (m.value)}
        <label class="model" class:selected={s.claudeModel === m.value}>
          <input type="radio" name="claude" value={m.value} bind:group={s.claudeModel} />
          <span class="m-text"><b>{m.label}</b><span class="muted small">{m.note}</span></span>
        </label>
      {/each}
    </div>
  {:else}
    <p class="muted small">Removes um/uh, capitalizes, adds a period, and applies your dictionary. No internet needed, no personality either.</p>
  {/if}
</section>

<section class="card group">
  <h2>Behavior</h2>
  <div class="lists-row">
    <span class="m-text">
      <b>When you run through several things</b>
      <span class="muted small">A bullet list when you run through several things, or your subjects gathered up when you bounce between them.</span>
    </span>
    <Segmented
      size="sm"
      label="Lists"
      value={s.lists}
      options={[
        { value: "ask", label: "Ask me" },
        { value: "auto", label: "Tidy it up" },
        { value: "never", label: "Keep as said" },
      ]}
      onchange={(v) => (s.lists = v as Settings["lists"])}
    />
  </div>
  <Toggle bind:checked={s.autoPaste} label="Paste automatically" hint="Otherwise Yap just copies it to your clipboard" />
  <Toggle bind:checked={s.restoreClipboard} label="Put my clipboard back" hint="Restores whatever you had copied after pasting" />
  <Toggle bind:checked={s.sounds} label="Sounds" hint="A soft blip when Yap starts and stops listening" />
</section>

{#if mac}
  <section class="card group">
    <h2>Permissions</h2>
    <div class="perm">
      <span class="m-text">
        <b>Accessibility</b>
        <span class="muted small">Lets Yap paste into other apps. Microphone access is asked for the first time you talk.</span>
      </span>
      {#if axOk}
        <span class="ready"><Icon name="check" size={14} />Allowed</span>
      {:else}
        <button class="btn primary sm" onclick={() => api.requestAccessibility()}><Icon name="shield" size={14} />Allow</button>
      {/if}
    </div>
  </section>
{/if}

<section class="card group">
  <h2>Library</h2>
  <p class="sub">Your words, dictionary, samples, rules and history in one file. Keep it in a folder that syncs to share it between computers, or export and import it by hand.</p>
  <div class="perm">
    <span class="m-text">
      <b>{s.libraryFolder ? "Syncing with" : "Library folder"}</b>
      {#if s.libraryFolder}
        <span class="folder">{s.libraryFolder}</span>
      {:else}
        <span class="muted small">Not set up. Pick a folder in Google Drive, iCloud or Synology.</span>
      {/if}
    </span>
    {#if s.libraryFolder}<button class="btn ghost sm" onclick={() => useFolder("")}>Stop</button>{/if}
    <button class="btn sm" onclick={chooseFolder}><Icon name="folder" size={14} />{s.libraryFolder ? "Change" : "Choose folder"}</button>
  </div>
  <div class="lib-actions">
    <button class="btn sm" onclick={exportLibrary}><Icon name="upload" size={14} />Export library…</button>
    <button class="btn sm" onclick={importLibrary}><Icon name="download" size={14} />Import library…</button>
    {#if libStatus}<span class="muted small">{libStatus}</span>{/if}
  </div>
</section>

<section class="card group">
  <div class="group-head">
    <div>
      <h2>Cloud sync</h2>
      <p class="sub">Keeps your library in Firebase so it reaches your other computers. It's encrypted here first, with a passphrase that never leaves this machine, so Firebase only ever holds a blob it can't read.</p>
    </div>
    <Toggle bind:checked={s.cloudSync} label="On" />
  </div>

  {#if s.cloudSync}
    {#if !cloud.registered}
      <p class="label">How do you want this computer set up?</p>
      <div class="choices">
        <div class="choice">
          <b>Just a passphrase</b>
          <span class="muted small">No account, nothing to sign in to. Any computer with the same passphrase finds the same library.</span>
          <button class="btn sm" onclick={usePassphraseOnly} disabled={cloudBusy}>Use a passphrase</button>
        </div>
        <div class="choice">
          <b>Sign in with Google</b>
          <span class="muted small">Your account keeps your library apart from everyone else's, so a weak passphrase can't expose it. Needs a Desktop OAuth client below.</span>
          <button class="btn sm" onclick={signIn} disabled={cloudBusy || !canSignIn}>Sign in with Google</button>
          {#if !canSignIn}<span class="muted small">Add a client ID and secret under Firebase details first.</span>{/if}
        </div>
      </div>
    {:else}
      <div class="perm">
        <span class="m-text">
          <b>{cloud.anonymous ? "Passphrase only" : "Signed in"}</b>
          <span class="muted small">{cloud.anonymous ? "This computer is registered without an account." : cloud.email}</span>
        </span>
        <button class="btn ghost sm" onclick={forget} disabled={cloudBusy}>Forget this computer</button>
      </div>
    {/if}

    <label class="label" for="pass">Passphrase {cloud.hasPassphrase ? "(set)" : ""}</label>
    <div class="key-row">
      <input
        id="pass"
        class="field"
        type={showPass ? "text" : "password"}
        placeholder={cloud.hasPassphrase ? "Saved on this computer" : `At least ${cloud.minPassphrase} characters`}
        bind:value={passphrase}
        autocomplete="off"
        spellcheck="false"
      />
      <button class="btn sm" onclick={() => (showPass = !showPass)}>{showPass ? "Hide" : "Show"}</button>
      <button class="btn sm" onclick={savePassphrase} disabled={cloudBusy || !passphrase.trim()}>Save</button>
    </div>
    <p class="muted small">
      Your library is encrypted with this before it leaves. Use the same passphrase on every computer, or they won't be able to read each other. There's no way to recover it: lose it and the cloud copy can't be opened, though everything here is untouched.
    </p>

    <div class="lib-actions">
      <button class="btn sm" onclick={syncNow} disabled={cloudBusy || !cloud.registered || !cloud.hasPassphrase}>
        <Icon name="refresh" size={14} />Sync now
      </button>
      {#if cloudStatus}<span class="muted small">{cloudStatus}</span>{/if}
      {#if cloudError}<span class="small err">{cloudError}</span>{/if}
    </div>

    <details class="setup">
      <summary>Firebase details</summary>
      <label class="label" for="fb-proj">Project ID</label>
      <input id="fb-proj" class="field" bind:value={s.firebaseProjectId} autocomplete="off" spellcheck="false" />
      <label class="label" for="fb-key">Web API key</label>
      <input id="fb-key" class="field" bind:value={s.firebaseApiKey} autocomplete="off" spellcheck="false" />
      <label class="label" for="g-id">Google client ID (Desktop app) — only to sign in</label>
      <input id="g-id" class="field" bind:value={s.googleClientId} autocomplete="off" spellcheck="false" />
      <label class="label" for="g-secret">Google client secret</label>
      <input id="g-secret" class="field" type="password" bind:value={s.googleClientSecret} autocomplete="off" spellcheck="false" />
    </details>
  {/if}
</section>

<section class="card group">
  <h2>List Formatting</h2>
  <p class="sub">Customize how Yap formats lists when you run through several things.</p>

  <div class="row-field">
    <label class="label" for="bullet">Bullet Style</label>
    <select id="bullet" class="field" bind:value={s.listPreferences.bulletStyle}>
      <option value="dash">- Dash</option>
      <option value="asterisk">* Asterisk</option>
      <option value="bullet">• Bullet point</option>
      <option value="number">1. Numbered</option>
      <option value="→">→ Arrow</option>
    </select>
    <span class="muted small">Choose how items appear in the list.</span>
  </div>

  <div class="row-field">
    <label class="label" for="intro">Intro Text (optional)</label>
    <input
      id="intro"
      class="field"
      type="text"
      placeholder="e.g., 'Here's what I said:'"
      bind:value={s.listPreferences.introText}
    />
    <span class="muted small">Text before the list. Leave empty for none.</span>
  </div>

  <div class="row-field">
    <label class="label" for="spacing">Item Spacing</label>
    <select id="spacing" class="field" bind:value={s.listPreferences.itemSpacing}>
      <option value="single">Single (compact)</option>
      <option value="double">Double (readable)</option>
    </select>
    <span class="muted small">Blank line between items.</span>
  </div>

  <Toggle bind:checked={s.listPreferences.allowNumbered} label="Allow numbered lists" hint="Use 1, 2, 3 instead of bullets when appropriate" />

  <div class="row-field">
    <label class="label" for="indent">Indent Size</label>
    <select id="indent" class="field" bind:value={s.listPreferences.indentSpaces}>
      <option value={0}>No indent</option>
      <option value={2}>2 spaces</option>
      <option value={4}>4 spaces</option>
    </select>
    <span class="muted small">Space for nested items (if grouped by subject).</span>
  </div>

  <div class="preview-box">
    <span class="label">Preview:</span>
    <pre class="preview">{generateListPreview(s.listPreferences)}</pre>
  </div>
</section>

<section class="card group">
  <h2>Settings Management</h2>
  <p class="sub">Export and import your settings for backup or company-wide deployment.</p>
  <div class="lib-actions">
    <button class="btn sm" onclick={exportSettings}>
      <Icon name="download" size={14} />Export settings…
    </button>
    <button class="btn sm" onclick={importSettings}>
      <Icon name="upload" size={14} />Import settings…
    </button>
  </div>
  {#if settingsStatus}<span class="muted small">{settingsStatus}</span>{/if}
</section>

<section class="card group">
  <h2>History</h2>
  <div class="perm">
    <span class="m-text">
      <b>{snap.history.length} dictation{snap.history.length === 1 ? "" : "s"} saved</b>
      <span class="muted small">Stored only on this device.</span>
    </span>
    <button class="btn sm danger" class:confirm={confirmClear} onclick={clearAll}>{confirmClear ? "Click again to clear" : "Clear history"}</button>
  </div>
</section>

<style>
  .title-row { display: flex; align-items: center; gap: 14px; }
  .saved { display: inline-flex; align-items: center; gap: 5px; color: var(--good); font-size: 13px; font-weight: 600; }
  .err { color: var(--bad); font-size: 13px; margin-top: 8px; }

  .group { padding: 22px 24px; margin-bottom: 14px; display: grid; gap: 12px; }
  .group h2 { font-size: 17px; font-weight: 650; }
  .sub { color: var(--ink-2); font-size: 13.5px; margin-top: -6px; }
  .group-head { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; }
  .group-head .sub { margin-top: 3px; }

  .models { display: grid; gap: 8px; }
  .model {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: 12px;
    border: 1px solid var(--line);
    background: var(--card-2);
    cursor: pointer;
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .model.selected { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); background: var(--card); }
  .model input { accent-color: var(--accent); margin: 0; }
  .m-text { flex: 1; display: grid; gap: 2px; }
  .ready { display: inline-flex; align-items: center; gap: 4px; color: var(--good); font-weight: 600; font-size: 12.5px; }

  .row-field { display: grid; grid-template-columns: 1fr; gap: 6px; max-width: 320px; }
  .row-field .label { margin: 0; }
  .presets { display: flex; gap: 8px; }
  .presets .on { border-color: var(--accent); color: var(--accent-2); }
  .grid2 { display: grid; grid-template-columns: 1.4fr 1fr; gap: 12px; }
  .key-row { display: flex; gap: 8px; align-items: center; margin-top: -6px; }
  .perm { display: flex; align-items: center; gap: 14px; }
  .lists-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding-bottom: 12px; border-bottom: 1px solid var(--line); }
  .err { color: var(--bad); }
  .choices { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 10px; margin: 4px 0 6px; }
  .choice {
    display: grid;
    gap: 7px;
    align-content: start;
    padding: 13px 14px;
    border-radius: 12px;
    background: var(--card-2);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .choice .btn { justify-self: start; }
  .setup { margin-top: 6px; border-top: 1px solid var(--line); padding-top: 12px; }
  .setup summary { cursor: pointer; font-size: 13px; font-weight: 560; color: var(--ink-2); }
  .setup summary:hover { color: var(--ink); }
  .lib-actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .folder { font: 12px var(--mono); color: var(--ink-2); word-break: break-all; }
  .danger { color: var(--bad); }
  .danger.confirm { background: var(--bad); color: #fff; border-color: transparent; }

  .preview-box { display: grid; gap: 6px; }
  .preview {
    background: var(--card-2);
    border: 1px solid var(--line);
    padding: 10px;
    border-radius: 6px;
    font-size: 12px;
    font-family: monospace;
    white-space: pre-wrap;
    word-wrap: break-word;
    margin: 0;
    color: var(--ink-2);
  }
</style>
