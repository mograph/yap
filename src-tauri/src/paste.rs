//! Puts text into whatever app has focus: clipboard + a simulated Cmd/Ctrl+V,
//! then puts the user's old clipboard back.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::mpsc;
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

/// Pastes into the focused app. Blocking; call off the main thread.
/// Returns Ok(false) when the text was only copied because paste permission is missing.
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
        let _ = tx.send(press_paste());
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(2)).map_err(|e| e.to_string())??;

    if let Some(prev) = previous {
        // Give the target app time to read the clipboard before swapping it back. Windows
        // delivers the keystroke to the other app's message loop and an Electron or browser
        // window can be slow to get to it, so it waits longer here than a Mac needs.
        // Use retry logic with backoff for reliability.
        let initial_settle = if cfg!(target_os = "windows") { 1500 } else { 500 };
        let max_retries = 3;
        let mut restored = false;

        for attempt in 0..max_retries {
            let delay = initial_settle + (attempt * 200);
            std::thread::sleep(Duration::from_millis(delay));

            // Try to restore the clipboard. Only do it if dictation is still there.
            match cb.get_text() {
                Ok(current) if current == text => {
                    // Text is still the dictation, so restore previous clipboard
                    match cb.set_text(prev.clone()) {
                        Ok(_) => {
                            restored = true;
                            eprintln!("yap: clipboard restored after {} attempt(s)", attempt + 1);
                            break;
                        }
                        Err(e) => {
                            eprintln!("yap: failed to restore clipboard (attempt {}): {}", attempt + 1, e);
                            continue; // Retry if set fails
                        }
                    }
                }
                Ok(_) => {
                    // App already modified/read the clipboard, don't restore
                    restored = true;
                    eprintln!("yap: app already used clipboard, skipping restore");
                    break;
                }
                Err(e) => {
                    // Clipboard error, retry
                    eprintln!("yap: clipboard read error (attempt {}): {}", attempt + 1, e);
                    continue;
                }
            }
        }

        if !restored {
            eprintln!("yap: clipboard restoration failed after {} attempts", max_retries);
        }
    }
    Ok(true)
}
