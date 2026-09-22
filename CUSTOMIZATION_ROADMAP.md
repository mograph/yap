# Yap Windows: Customization & List Formatting Roadmap

## Current State vs. Desired State

### What Users Can Already Customize

✅ **Talk Key**: Right-Ctrl, Right-Alt, Ctrl, Alt, or custom shortcut
✅ **Auto-Paste**: On/Off
✅ **Restore Clipboard**: On/Off
✅ **List Behavior**: Ask / Auto / Never
✅ **Cleanup Rules**: Per-kind (Do / Suggest / Leave)
- Filler words, corrections, punctuation, grammar, slang, rewording, swearing, formatting, dictionary

❌ **List Format**: No granular control over how lists are formatted
❌ **Customizable Language**: No way to customize list intro text or labels
❌ **Visibility**: No clear "is paste working?" feedback
❌ **Per-App Behavior**: No way to set different paste strategies for different apps

---

## Phase 1: List Formatting Controls (High Impact, Medium Effort)

### Goal
Users can customize bullet style, intro text, line spacing, and numbered lists.

### New Settings Structure (store.rs)

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct ListPreferences {
    /// Bullet style: "dash", "asterisk", "bullet", "number", or custom like "→"
    pub bullet_style: String,
    /// Intro text before list: "Here's what I said:", "You mentioned:", or ""
    pub intro_text: String,
    /// Between-item spacing: "single" (one line) or "double" (two lines)
    pub item_spacing: String,
    /// Allow numbered lists (1, 2, 3) instead of bullets
    pub allow_numbered: bool,
    /// Indent sub-items by N spaces (for nested grouping)
    pub indent_spaces: u8,
}

impl Default for ListPreferences {
    fn default() -> Self {
        Self {
            bullet_style: "dash".into(),         // -
            intro_text: "You said:".into(),      // Optional
            item_spacing: "single".into(),
            allow_numbered: false,
            indent_spaces: 2,
        }
    }
}
```

### Add to main Settings struct (store.rs)

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    // ... existing fields ...
    pub lists: String,  // "ask", "auto", "never"
    pub list_preferences: ListPreferences,  // NEW
}
```

### Update polish.rs (list formatting logic)

Currently, list formatting happens in `polish.rs`. Modify it to use the new preferences:

```rust
fn format_list(items: Vec<String>, prefs: &ListPreferences) -> String {
    let mut result = String::new();
    
    // Add intro text if provided
    if !prefs.intro_text.is_empty() {
        result.push_str(&prefs.intro_text);
        result.push('\n');
    }
    
    let bullet = match prefs.bullet_style.as_str() {
        "dash" => "-",
        "asterisk" => "*",
        "bullet" => "•",
        "number" => "",  // Handled separately
        custom => custom,
    };
    
    for (i, item) in items.iter().enumerate() {
        let spacing = if prefs.item_spacing == "double" { "\n" } else { "" };
        
        let prefix = if prefs.allow_numbered && prefs.bullet_style == "number" {
            format!("{}. ", i + 1)
        } else {
            format!("{} ", bullet)
        };
        
        result.push_str(&prefix);
        result.push_str(item);
        result.push('\n');
        if !spacing.is_empty() && i < items.len() - 1 {
            result.push_str(&spacing);
        }
    }
    
    result
}
```

### UI: Settings.svelte - New "Lists" Section

Add this section after the existing "Behavior" section:

```svelte
<section class="card group">
  <div class="group-head">
    <div>
      <h2>List Formatting</h2>
      <p class="sub">When Yap detects and formats lists for you.</p>
    </div>
  </div>

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
    <PreviewList items={previewItems} prefs={s.listPreferences} />
  </div>
</section>
```

### Preview Component (PreviewList.svelte)

```svelte
<script lang="ts">
  import type { ListPreferences } from "../lib/api";

  let { items = ["First item", "Second item", "Third item"], prefs } = $props();

  const preview = $derived.by(() => {
    let result = "";
    if (prefs.introText) result += prefs.introText + "\n";
    
    for (let i = 0; i < items.length; i++) {
      const spacing = prefs.itemSpacing === "double" && i > 0 ? "\n" : "";
      
      let prefix = "";
      switch (prefs.bulletStyle) {
        case "dash": prefix = "-"; break;
        case "asterisk": prefix = "*"; break;
        case "bullet": prefix = "•"; break;
        case "number": prefix = String(i + 1) + "."; break;
        default: prefix = prefs.bulletStyle;
      }
      
      result += spacing + prefix + " " + items[i] + "\n";
    }
    return result;
  });
</script>

<pre class="preview">{preview}</pre>

<style>
  .preview {
    background: var(--card-2);
    border: 1px solid var(--line);
    padding: 12px;
    border-radius: 6px;
    font-size: 13px;
    font-family: monospace;
    white-space: pre-wrap;
    word-wrap: break-word;
  }
</style>
```

---

## Phase 2: Paste Feedback & Visibility (High Impact, Low Effort)

### Goal
Users always know if paste succeeded, failed, or manual action is needed.

### Add to DictationCard.svelte

When auto-paste happens:
```svelte
<script>
  let pasteStatus = $state("");
  
  async function copy() {
    await api.copy(item.text);
    pasteStatus = "Copied to clipboard";
    setTimeout(() => (pasteStatus = ""), 2000);
  }
  
  async function pasteAndShow() {
    try {
      pasteStatus = "Pasting…";
      const result = await api.paste(item.text);
      pasteStatus = result ? "✓ Pasted" : "Copied (paste manually: Ctrl+V)";
      setTimeout(() => (pasteStatus = ""), 2500);
    } catch (e) {
      pasteStatus = "✗ Error: " + String(e);
      setTimeout(() => (pasteStatus = ""), 3000);
    }
  }
</script>

{#if pasteStatus}
  <div class="paste-status" class:success={pasteStatus.includes("✓")}>
    {pasteStatus}
  </div>
{/if}
```

### Add to Home.svelte (Overlay display)

Show current paste setting and status:
```svelte
<div class="status-bar">
  <span class="setting-badge">
    {#if snap.settings.autoPaste}
      Auto-paste: ON
    {:else}
      Copy-only mode
    {/if}
  </span>
  {#if lastPasteStatus}
    <span class="paste-feedback" class:success={lastPasteStatus.includes("✓")}>
      {lastPasteStatus}
    </span>
  {/if}
</div>
```

### Toast Notification System

Create Toast.svelte component:
```svelte
<script lang="ts">
  import { appToasts } from "../lib/state.svelte";
</script>

<div class="toast-container">
  {#each appToasts as toast, i (i)}
    <div class="toast" class:success={toast.type === "success"} class:error={toast.type === "error"}>
      {toast.message}
    </div>
  {/each}
</div>

<style>
  .toast-container {
    position: fixed;
    bottom: 20px;
    right: 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    pointer-events: none;
  }
  .toast {
    background: #333;
    color: #fff;
    padding: 12px 16px;
    border-radius: 6px;
    font-size: 13px;
    pointer-events: auto;
    animation: slideIn 0.3s ease-out;
  }
  .toast.success { background: #28a745; }
  .toast.error { background: #dc3545; }
  @keyframes slideIn {
    from { transform: translateX(400px); opacity: 0; }
    to { transform: translateX(0); opacity: 1; }
  }
</style>
```

---

## Phase 3: Per-App Paste Strategies (Medium Impact, Medium Effort)

### Goal
Allow different paste behavior for specific apps (Discord, Slack, Gmail, etc.)

### New Settings Structure

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppPasteStrategy {
    pub app_window_title_pattern: String,  // Regex: ".*Discord.*" or "Gmail"
    pub strategy: String,  // "auto" (default) / "copy-only" / "manual"
    pub paste_delay_ms: u64,  // Override global delay
}

#[derive(Serialize, Deserialize, Clone, Debug, default)]
pub struct Settings {
    // ... existing ...
    pub paste_strategies: Vec<AppPasteStrategy>,
}
```

### Detection & Application

In lib.rs, before pasting:

```rust
fn get_paste_strategy(settings: &Settings) -> String {
    let fg_window = get_foreground_window_title();  // Get active window title
    
    for strategy in &settings.paste_strategies {
        if regex::Regex::new(&strategy.app_window_title_pattern)
            .map(|r| r.is_match(&fg_window))
            .unwrap_or(false)
        {
            return strategy.strategy.clone();
        }
    }
    
    "auto".to_string()  // Default
}
```

### UI: App-Specific Paste Settings

```svelte
<section class="card group">
  <h2>App-Specific Paste Behavior</h2>
  <p class="sub">Override paste strategy for specific apps.</p>

  <button class="btn sm" onclick={addStrategy}>
    <Icon name="plus" size={14} />Add app rule…
  </button>

  {#each s.pasteStrategies as strategy, i (i)}
    <div class="strategy-card">
      <label>App name (window title)</label>
      <input class="field" bind:value={strategy.appWindowTitlePattern} placeholder="e.g., Discord" />
      
      <label>Paste strategy</label>
      <select class="field" bind:value={strategy.strategy}>
        <option value="auto">Auto (smart retry)</option>
        <option value="copy-only">Copy only, user does Ctrl+V</option>
        <option value="manual">Never auto-paste</option>
      </select>
      
      <label>Paste delay (ms)</label>
      <input class="field" type="number" bind:value={strategy.pasteDelayMs} />
      
      <button class="btn ghost sm danger" onclick={() => removeStrategy(i)}>
        <Icon name="trash" size={13} />Remove
      </button>
    </div>
  {/each}
</section>
```

---

## Phase 4: List Formatting Presets (Low Effort, Nice to Have)

### Preset Configurations

```rust
pub const LIST_PRESETS: &[(&str, ListPreferences)] = &[
    (
        "Slack",
        ListPreferences {
            bullet_style: "dash".into(),
            intro_text: "".into(),  // No intro for Slack
            item_spacing: "single".into(),
            allow_numbered: false,
            indent_spaces: 0,
        },
    ),
    (
        "Email",
        ListPreferences {
            bullet_style: "number".into(),
            intro_text: "Here's what I said:".into(),
            item_spacing: "double".into(),
            allow_numbered: true,
            indent_spaces: 2,
        },
    ),
    (
        "Meeting Notes",
        ListPreferences {
            bullet_style: "dash".into(),
            intro_text: "Action items:".into(),
            item_spacing: "double".into(),
            allow_numbered: false,
            indent_spaces: 4,
        },
    ),
];
```

### UI: Quick Preset Buttons

```svelte
<div class="presets">
  {#each LIST_PRESETS as [name, prefs] (name)}
    <button
      class="btn sm"
      class:active={s.listPreferences === prefs}
      onclick={() => (s.listPreferences = prefs)}
    >
      {name}
    </button>
  {/each}
  <button class="btn sm" onclick={() => (showCustom = true)}>Custom</button>
</div>
```

---

## Implementation Checklist

### Phase 1: List Formatting
- [ ] Add ListPreferences struct to store.rs
- [ ] Update Settings struct
- [ ] Modify polish.rs to use preferences
- [ ] Add UI controls in Settings.svelte
- [ ] Create PreviewList.svelte component
- [ ] Update DictationCard to show applied format
- [ ] Test with various settings

### Phase 2: Paste Feedback
- [ ] Create Toast component
- [ ] Add paste status to DictationCard
- [ ] Add status badge to Home.svelte overlay
- [ ] Connect to actual paste result events
- [ ] Test success/failure scenarios

### Phase 3: Per-App Strategies
- [ ] Add AppPasteStrategy struct
- [ ] Implement window title detection
- [ ] Add strategy selection logic
- [ ] Create UI for managing rules
- [ ] Test with Discord, Slack, Gmail, Teams

### Phase 4: Presets
- [ ] Define preset configurations
- [ ] Add preset buttons to UI
- [ ] Allow saving custom presets
- [ ] Document each preset

---

## Distribution Benefits

With these changes, you can tell your company:

✅ **"Customize everything"** — Bullet style, intro text, spacing, numbering
✅ **"Know it's working"** — See feedback when paste succeeds/fails
✅ **"Works with your apps"** — Per-app paste strategies for Discord, Slack, Gmail, etc.
✅ **"Pre-configured"** — Import company defaults with all settings pre-set
✅ **"Granular control"** — Turn customization off if users don't want it

---

## Timeline Estimate

- Phase 1 (List Formatting): 3-4 hours
- Phase 2 (Paste Feedback): 1-2 hours
- Phase 3 (Per-App): 2-3 hours
- Phase 4 (Presets): 1 hour
- **Total: 7-10 hours of implementation + testing**

**Recommend doing Phase 1 + 2 before company rollout (both are high-value, low-risk).**
