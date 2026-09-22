# Windows Copy/Paste Fixes & Distribution Guide

## Critical Fix #1: Clipboard Restoration Race Condition

### Problem
The current logic waits 1200ms, then checks if the dictation is still on the clipboard. But:
- Some apps (Slack Web, Discord web) have slow event loops
- Network-based clipboards (Teams) may not read immediately
- The check doesn't differentiate between "clipboard is empty" and "app error reading"

### Current Code (paste.rs:73-88)
```rust
if let Some(prev) = previous {
    let settle = if cfg!(target_os = "windows") { 1200 } else { 400 };
    std::thread::sleep(Duration::from_millis(settle));
    match cb.get_text() {
        Ok(current) if current == text => {
            let _ = cb.set_text(prev);  // ← Fails silently
        }
        _ => {}
    }
}
```

### Proposed Fix
Replace with **retry logic + fallback**:
```rust
if let Some(prev) = previous {
    let settle = if cfg!(target_os = "windows") { 1500 } else { 500 };
    let max_retries = 3;
    let mut restored = false;
    
    for attempt in 0..max_retries {
        std::thread::sleep(Duration::from_millis(settle + (attempt * 200) as u64));
        match cb.get_text() {
            Ok(current) if current == text => {
                match cb.set_text(prev.clone()) {
                    Ok(_) => {
                        restored = true;
                        break;
                    }
                    Err(_) => continue, // Retry if set fails
                }
            }
            Ok(_) => break,  // App already modified clipboard, don't restore
            Err(_) => continue, // Clipboard error, retry
        }
    }
    
    if !restored {
        // Log error or emit event so UI can show "Couldn't restore your clipboard"
        eprintln!("yap: clipboard restoration failed after {} attempts", max_retries);
    }
}
```

**Changes:**
- Increase initial settle time to 1500ms on Windows
- Retry up to 3 times with progressive backoff
- Differentiate between "app already used clipboard" vs "clipboard error"
- Track success/failure for UI feedback

---

## Critical Fix #2: Fallback Paste Strategy for Web Apps

### Problem
On Slack Web, Discord, Gmail web, the Ctrl+V keystroke is sent but doesn't trigger. This is because:
- Browser-based apps intercept paste events differently
- The keystroke timing may conflict with app's own input handlers
- No fallback if the first attempt fails

### Current Code (paste.rs:32-47)
```rust
fn press_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let modifier = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };
    #[cfg(target_os = "windows")]
    let v = Key::V;
    #[cfg(not(target_os = "windows"))]
    let v = Key::Unicode('v');
    enigo.key(modifier, Direction::Press).map_err(|e| e.to_string())?;
    let res = enigo.key(v, Direction::Click).map_err(|e| e.to_string());
    enigo.key(modifier, Direction::Release).map_err(|e| e.to_string())?;
    res
}
```

### Proposed Fix
Add **retry + longer hold strategy**:
```rust
fn press_paste_with_fallback() -> Result<bool, String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let modifier = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };
    #[cfg(target_os = "windows")]
    let v = Key::V;
    #[cfg(not(target_os = "windows"))]
    let v = Key::Unicode('v');
    
    // Strategy 1: Standard Ctrl+V click
    enigo.key(modifier, Direction::Press).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(5));
    enigo.key(v, Direction::Click).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(5));
    enigo.key(modifier, Direction::Release).map_err(|e| e.to_string())?;
    
    // Strategy 2: Retry with longer hold (for slow/web apps)
    std::thread::sleep(Duration::from_millis(100));
    enigo.key(modifier, Direction::Press).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(50));
    enigo.key(v, Direction::Press).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(50));
    enigo.key(v, Direction::Release).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(50));
    enigo.key(modifier, Direction::Release).map_err(|e| e.to_string())?;
    
    // Success (both strategies attempted)
    Ok(true)
}
```

**Usage in paste():**
```rust
pub fn paste(app: &AppHandle, text: &str, restore: bool) -> Result<bool, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?;
    let previous = if restore { cb.get_text().ok() } else { None };
    cb.set_text(text.to_string()).map_err(|e| e.to_string())?;
    if !accessibility_ok() {
        return Ok(false);
    }
    std::thread::sleep(Duration::from_millis(30));

    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(press_paste_with_fallback());
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(2)).map_err(|e| e.to_string())??;

    // Restore clipboard (with new retry logic from Fix #1)
    if let Some(prev) = previous {
        // ... (use new retry logic here)
    }
    Ok(true)
}
```

---

## Fix #3: Settings Export/Import for Distribution

### Add to store.rs

```rust
#[tauri::command]
pub fn export_settings(app: AppHandle) -> Result<Settings, String> {
    let st = app.state::<AppState>();
    Ok(st.settings.lock().unwrap().clone())
}

#[tauri::command]
pub fn import_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let st = app.state::<AppState>();
    *st.settings.lock().unwrap() = settings.clone();
    st.save_settings()
}

#[tauri::command]
pub fn save_settings_to_file(settings: Settings, path: String) -> Result<(), String> {
    std::fs::write(&path, serde_json::to_string_pretty(&settings)?)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_settings_from_file(path: String) -> Result<Settings, String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read settings file: {e}"))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Invalid settings JSON: {e}"))
}
```

### Add to UI (Settings.svelte)

```svelte
<section class="card group">
  <h2>Settings Management</h2>
  <p class="sub">For company deployment and backup.</p>
  <div class="lib-actions">
    <button class="btn sm" onclick={exportSettings}>
      <Icon name="download" size={14} />Export settings…
    </button>
    <button class="btn sm" onclick={importSettings}>
      <Icon name="upload" size={14} />Import settings…
    </button>
  </div>
</section>
```

### Handler functions

```typescript
async function exportSettings() {
  const path = await save({
    defaultPath: "yap-settings.json",
    filters: [{ name: "Yap Settings", extensions: ["json"] }],
  });
  if (!path) return;
  try {
    const settings = await api.settings();
    await api.saveSettingsToFile(settings, path);
    libStatus = "Settings exported";
  } catch (e) {
    libStatus = String(e);
  }
}

async function importSettings() {
  const path = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Yap Settings", extensions: ["json"] }],
  });
  if (typeof path !== "string") return;
  try {
    const settings = await api.loadSettingsFromFile(path);
    await api.saveSettings(settings);
    s = settings; // Update UI
    libStatus = "Settings imported";
  } catch (e) {
    libStatus = String(e);
  }
}
```

### First-Launch Default Behavior

On app startup, check for `%APPDATA%\Yap\settings.default.json`. If present, offer to import as defaults:

```rust
fn load_default_settings_if_present(data_dir: &Path) -> Option<Settings> {
    let default_path = data_dir.join("settings.default.json");
    if default_path.exists() {
        std::fs::read_to_string(&default_path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
    } else {
        None
    }
}
```

---

## Deployment Strategy for Your Company

### Step 1: Create Company Settings JSON
```json
{
  "trigger": "right-ctrl",
  "shortcut": "Ctrl+Shift+Space",
  "sttEngine": "local",
  "localModel": "large-v3-turbo-q5_0",
  "language": "auto",
  "brain": "claude",
  "claudeModel": "claude-opus-5",
  "autoPaste": true,
  "restoreClipboard": true,
  "sounds": true,
  "lists": "ask"
}
```

### Step 2: Package with Installer
Include in installer package or shared folder:
```
C:\Program Files\Yap\defaults\settings.default.json
```

### Step 3: User First Launch
When Yap starts for the first time:
1. Check if `%APPDATA%\Yap\settings.default.json` exists
2. If yes → Show dialog: "Import company defaults?"
3. If yes → Load those settings, user can customize later
4. If no → Show onboarding as normal

---

## Testing Checklist Before Distribution

- [ ] Slack Web: Paste in thread reply box (with restore clipboard ON)
- [ ] Discord Web: Paste in text box (with restore clipboard ON)
- [ ] Gmail: Compose new email → auto-paste (check clipboard restored)
- [ ] Teams Web: Paste in chat (with restore clipboard ON)
- [ ] Notepad: Auto-paste works, clipboard restored
- [ ] Rapid dictations: 3 in a row, check clipboard integrity
- [ ] Settings export/import: Export, modify, import, verify changes
- [ ] First launch with settings.default.json: Verify defaults applied
- [ ] Manual copy button: Works in all contexts

---

## Summary

| Issue | Fix | Effort | Risk |
|-------|-----|--------|------|
| Clipboard race condition | Retry with backoff (3x) | Low | Low |
| Paste not triggering in web apps | Fallback with longer holds | Medium | Low |
| No settings distribution | Export/import + default file | Medium | Very Low |

**Recommend implementing all three before company rollout.**
