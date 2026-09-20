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

fn press_paste() -> Result<(), String> {
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
    enigo.key(modifier, Direction::Press).map_err(|e| e.to_string())?;
    let res = enigo.key(v, Direction::Click).map_err(|e| e.to_string());
    enigo.key(modifier, Direction::Release).map_err(|e| e.to_string())?;
    res
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
        let settle = if cfg!(target_os = "windows") { 1200 } else { 400 };
        std::thread::sleep(Duration::from_millis(settle));
        // Only take the dictation back off the clipboard if it's still what's on there, so
        // copying something yourself in the meantime survives. Turning "restore clipboard"
        // off in Settings is what keeps the dictation there to paste again afterwards.
        match cb.get_text() {
            Ok(current) if current == text => {
                let _ = cb.set_text(prev);
            }
            _ => {}
        }
    }
    Ok(true)
}
