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

/// Sends one Ctrl/Cmd+V as four distinct, separately-held steps (press modifier, press V,
/// release V, release modifier) rather than a single `Direction::Click` for V, which press-
/// and-releases with no gap in between: some apps - web apps (Discord, Slack Web, Gmail) and,
/// it turns out, Windows Terminal / VS Code's integrated terminal among native ones - need V
/// to actually stay down for a moment, with the modifier clearly down first, to register the
/// chord at all. Sending the gesture *twice* was tried before and just double-pasted in
/// anything that handled the first one fine, since there's no way to detect from here whether
/// the first one "worked" - so this is one gesture, just a more deliberate one. The modifier
/// is always released even if a V step fails, since leaving Ctrl stuck down at the OS level
/// would corrupt every keystroke the user types next.
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
    let hold = if cfg!(target_os = "windows") { 50 } else { 15 };

    enigo.key(modifier, Direction::Press).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(hold));
    let pressed = enigo.key(v, Direction::Press);
    std::thread::sleep(Duration::from_millis(hold));
    let released = enigo.key(v, Direction::Release);
    std::thread::sleep(Duration::from_millis(hold));
    enigo.key(modifier, Direction::Release).map_err(|e| e.to_string())?;
    pressed.and(released).map_err(|e| e.to_string())
}

pub fn copy(text: &str) -> Result<(), String> {
    let _guard = paste_lock().lock().unwrap();
    // A manual copy means the user has taken deliberate control of the clipboard, so any
    // restore a pending dictation still owes is now moot - bump the generation so that
    // restore sees it's been superseded and leaves this alone instead of clobbering it later.
    {
        let mut st = restore_state().lock().unwrap();
        st.generation += 1;
        st.original = None;
    }
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

/// Serializes "put text on the clipboard, then send Ctrl+V" across dictations. The generation
/// counter above only protects the later *restore* step - it does nothing to stop a second
/// dictation's `set_text` from landing while the first dictation's Ctrl+V is still sitting in
/// the target window's message queue, unprocessed. When that happens, the first paste ends up
/// reading whatever the second dictation just wrote instead of its own text: the app the first
/// dictation was pasted into silently gets the wrong content. Holding this lock through a short
/// settle after the keystrokes are sent (not the full clipboard-restore delay) keeps rapid
/// dictations from racing each other, while each one's "Pasted" status still returns promptly.
static PASTE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn paste_lock() -> &'static Mutex<()> {
    PASTE_LOCK.get_or_init(|| Mutex::new(()))
}

/// Pastes into the focused app. Blocking; call off the main thread.
/// Returns Ok(false) when the text was only copied because paste permission is missing.
pub fn paste(app: &AppHandle, text: &str, restore: bool) -> Result<bool, String> {
    let _guard = paste_lock().lock().unwrap();
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?;

    if !accessibility_ok() {
        // Still worth putting the text on the clipboard so the user can paste it by hand, but
        // nothing is about to be pasted, so there's no restore to track - claiming a generation
        // here would leave it stuck with no restore thread ever spawned to resolve it.
        cb.set_text(text.to_string()).map_err(|e| e.to_string())?;
        return Ok(false);
    }

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
    std::thread::sleep(Duration::from_millis(30));

    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(press_paste());
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(2)).map_err(|e| e.to_string())??;

    // Give the target app a moment to actually read the clipboard before a queued-up next
    // dictation is allowed to overwrite it.
    let post_settle = if cfg!(target_os = "windows") { 250 } else { 150 };
    std::thread::sleep(Duration::from_millis(post_settle));

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
                // Hold the lock across the write itself: dropping it first would let a
                // paste() call that's blocked claiming the next generation slip in between
                // this check and the write, finish an entire dictation, and then have this
                // stale restore stomp its freshly-pasted clipboard right after.
                match cb.set_text(prev) {
                    Ok(_) => {
                        st.original = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the real system clipboard, so it saves whatever was really there and puts it
    /// back on drop - including if an assertion below panics - rather than leaving the dev
    /// machine's clipboard in a test state.
    struct RestoreGuard(Option<String>);
    impl Drop for RestoreGuard {
        fn drop(&mut self) {
            if let Some(s) = self.0.take() {
                let _ = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(s));
            }
            let mut st = restore_state().lock().unwrap();
            st.generation = 0;
            st.original = None;
        }
    }

    /// The bug this guards against: dictation A pastes and claims a restore; before its
    /// restore fires, dictation B lands and claims the next generation. A's restore must then
    /// see it's been superseded and leave B's paste alone - not overwrite it with the old
    /// content, which is exactly the "pulls the last one instead of the latest" bug reported.
    /// `restore_clipboard` sleeps for real (same multi-second settle it uses in production)
    /// before each check, so this test takes a few seconds - that's the cost of exercising the
    /// real timing path rather than a faked one.
    #[test]
    fn stale_restore_does_not_stomp_a_newer_dictation_or_a_manual_copy() {
        let mut cb = arboard::Clipboard::new().unwrap();
        let _guard = RestoreGuard(cb.get_text().ok());

        // Dictation A pastes "A", claiming generation 1 and snapshotting the real clipboard.
        {
            let mut st = restore_state().lock().unwrap();
            st.generation = 1;
            st.original = Some("ORIGINAL".into());
        }
        // Dictation B lands before A's restore fires and claims the next generation.
        cb.set_text("B".to_string()).unwrap();
        restore_state().lock().unwrap().generation = 2;

        // A's stale restore must see it's been superseded and leave B's paste alone.
        restore_clipboard(1, "A");
        assert_eq!(cb.get_text().unwrap(), "B", "a stale restore must not stomp a newer paste");

        // A manual copy while a (different) restore is still pending must cancel it.
        {
            let mut st = restore_state().lock().unwrap();
            st.generation = 3;
            st.original = Some("STALE".into());
        }
        copy("manual").unwrap();
        assert_eq!(cb.get_text().unwrap(), "manual");
        restore_clipboard(3, "whatever was pasted");
        assert_eq!(cb.get_text().unwrap(), "manual", "a stale restore must not override a manual copy");
    }
}
