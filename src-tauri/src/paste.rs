//! Puts text into whatever app has focus: clipboard + a simulated Cmd/Ctrl+V,
//! then puts the user's old clipboard back.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;
use tauri::AppHandle;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

/// macOS only lets apps send keystrokes once they're allowed under Accessibility.
pub fn accessibility_ok() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        AXIsProcessTrusted()
    }
    #[cfg(not(target_os = "macos"))]
    true
}

/// Creating an Enigo on macOS pops the system "allow Accessibility" prompt if needed.
pub fn request_accessibility(app: &AppHandle) {
    let _ = app.run_on_main_thread(|| {
        let _ = Enigo::new(&Settings::default());
    });
}

fn press_paste() -> Result<bool, String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let modifier = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };
    // Windows sends a `Unicode` key as a typed character, which arrives as text rather than
    // as a keypress, so the Ctrl a paste needs is never applied to it: the app being pasted
    // into sees a stray "v" at most. The V virtual key is a real keypress and Ctrl sticks to
    // it. macOS maps `Unicode` to a keycode, where ⌘ applies, so it stays as it was.
    #[cfg(target_os = "windows")]
    let v = Key::V;
    #[cfg(not(target_os = "windows"))]
    let v = Key::Unicode('v');

    // Strategy 1: Standard Ctrl+V click (quick)
    enigo.key(modifier, Direction::Press).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(5));
    enigo.key(v, Direction::Click).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(5));
    enigo.key(modifier, Direction::Release).map_err(|e| e.to_string())?;

    // Strategy 2 (Windows only): Retry with longer hold for web apps (Discord, Slack Web, Gmail)
    #[cfg(target_os = "windows")]
    {
        std::thread::sleep(Duration::from_millis(100));
        enigo.key(modifier, Direction::Press).map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(50));
        enigo.key(v, Direction::Press).map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(50));
        enigo.key(v, Direction::Release).map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(50));
        enigo.key(modifier, Direction::Release).map_err(|e| e.to_string())?;
    }

    Ok(true)
}

pub fn copy(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut cb| cb.set_text(text.to_string()))
        .map_err(|e| format!("Clipboard unavailable: {e}"))
}

/// Coordinates clipboard restores across back-to-back dictations. Each `paste()` used to
/// snapshot "whatever's on the clipboard right now" and restore it on its own timer -
/// harmless in isolation, but if a second dictation lands before the first one's restore
/// fires, the second dictation's "previous" is really just the first dictation's pasted
/// text, and a slow first restore can land after the second paste and stomp it with that
/// stale text. Tracking a shared generation means only the newest paste's restore actually
/// writes to the clipboard, and it always writes back the *real* pre-dictation content.
struct RestoreState {
    generation: u64,
    original: Option<String>,
}

static RESTORE: OnceLock<Mutex<RestoreState>> = OnceLock::new();

fn restore_state() -> &'static Mutex<RestoreState> {
    RESTORE.get_or_init(|| Mutex::new(RestoreState { generation: 0, original: None }))
}

/// Pastes into the focused app. Blocking; call off the main thread.
/// Returns Ok(false) when the text was only copied because paste permission is missing.
pub fn paste(app: &AppHandle, text: &str, restore: bool) -> Result<bool, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?;

    // Claim the next generation. If an earlier dictation's restore hasn't landed yet, keep
    // its snapshot of the real original clipboard instead of grabbing the current clipboard
    // (which would just be that earlier dictation's own pasted text).
    let generation = if restore {
        let mut st = restore_state().lock().unwrap();
        st.generation += 1;
        if st.original.is_none() {
            st.original = cb.get_text().ok();
        }
        Some(st.generation)
    } else {
        None
    };

    cb.set_text(text.to_string()).map_err(|e| e.to_string())?;
    if !accessibility_ok() {
        return Ok(false);
    }
    std::thread::sleep(Duration::from_millis(30));

    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(press_paste());
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(2)).map_err(|e| e.to_string())??;

    if let Some(generation) = generation {
        // Restore happens on its own thread so a slow target app doesn't hold up the
        // "Pasted" status the user sees.
        let text = text.to_string();
        std::thread::spawn(move || restore_clipboard(generation, &text));
    }
    Ok(true)
}

/// Puts the real pre-dictation clipboard content back once the target app has had time to
/// read the paste. Windows delivers the keystroke to the other app's message loop and an
/// Electron or browser window (Slack, Discord, Teams, Gmail) can be slow to get to it, so
/// this errs long: restoring a couple seconds late is harmless, restoring early means the
/// target app can end up pasting the old clipboard instead of the dictation.
fn restore_clipboard(generation: u64, text: &str) {
    let Ok(mut cb) = arboard::Clipboard::new() else { return };
    let initial_settle = if cfg!(target_os = "windows") { 2500 } else { 800 };
    let max_retries = 4;

    for attempt in 0..max_retries {
        std::thread::sleep(Duration::from_millis(initial_settle + attempt * 400));

        let mut st = restore_state().lock().unwrap();
        if st.generation != generation {
            // A newer dictation has taken over the chain and inherited our snapshot; it'll
            // restore it when it's done. Nothing left for us to do.
            eprintln!("yap: superseded by a newer dictation, skipping restore");
            return;
        }

        match cb.get_text() {
            Ok(current) if current == text => {
                let Some(prev) = st.original.clone() else { return };
                drop(st);
                match cb.set_text(prev) {
                    Ok(_) => {
                        let mut st = restore_state().lock().unwrap();
                        if st.generation == generation {
                            st.original = None;
                        }
                        eprintln!("yap: clipboard restored after {} attempt(s)", attempt + 1);
                        return;
                    }
                    Err(e) => {
                        eprintln!("yap: failed to restore clipboard (attempt {}): {}", attempt + 1, e);
                    }
                }
            }
            Ok(_) => {
                // Something else (a manual copy, another dictation) already took the
                // clipboard - leave it alone and let the next dictation start fresh.
                st.original = None;
                eprintln!("yap: clipboard changed since paste, skipping restore");
                return;
            }
            Err(e) => {
                eprintln!("yap: clipboard read error (attempt {}): {}", attempt + 1, e);
            }
        }
    }
    eprintln!("yap: clipboard restoration failed after {} attempts", max_retries);
}
