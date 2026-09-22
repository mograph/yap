mod cloud;
mod library;
mod polish;
mod store;
mod stt;
#[cfg(desktop)]
mod audio;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod modkey;
#[cfg(desktop)]
mod paste;
#[cfg(test)]
mod tests;

use serde::Serialize;
use std::path::{Path, PathBuf};
#[cfg(desktop)]
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
#[cfg(desktop)]
use std::time::{Duration, Instant};
use store::{Dictation, Profile, Settings};
use tauri::{AppHandle, Emitter, Manager, State};

const HISTORY_LIMIT: usize = 2000;

#[derive(Clone, Copy)]
enum Phase {
    Idle,
    #[cfg_attr(mobile, allow(dead_code))]
    Listening {
        #[cfg(desktop)]
        since: Instant,
        /// Hands-free: keep listening until the talk key is tapped again.
        locked: bool,
        /// Paste into the focused app when done (false when started from Yap's own window).
        paste: bool,
        /// A quick press-and-release of a key combo switches to hands-free. Bare modifier
        /// keys use a double-tap for that instead.
        tap_locks: bool,
    },
    #[cfg_attr(mobile, allow(dead_code))]
    Working,
    /// Waiting for you to pick the tidier version or "as said".
    #[cfg_attr(mobile, allow(dead_code))]
    Choosing,
}

/// Everything that can start, finish or cancel a dictation. Handled one at a time on the
/// dictation thread, so the UI thread never waits on the mic, the model or a lock.
#[cfg(desktop)]
pub(crate) enum Input {
    Down { at: Instant, tap_locks: bool },
    Up { at: Instant },
    /// A clean tap of a bare modifier talk key
    Tap { at: Instant },
    /// The mic button in Yap's own window
    Toggle,
    Cancel,
}

pub struct AppState {
    data_dir: PathBuf,
    settings: Mutex<Settings>,
    profile: Mutex<Profile>,
    history: Mutex<Vec<Dictation>>,
    /// IDs of dictations you deleted, so a synced library doesn't bring them back.
    deleted: Mutex<Vec<String>>,
    /// Google sign-in and the passphrase your library is encrypted with. Owner-only on disk.
    account: Mutex<cloud::Account>,
    phase: Mutex<Phase>,
    #[cfg(desktop)]
    recorder: Arc<audio::Recorder>,
    #[cfg(desktop)]
    stt: Arc<stt::Local>,
    #[cfg(desktop)]
    input: Mutex<mpsc::Sender<Input>>,
    /// Answers the tidier-version question: true = take it.
    #[cfg(desktop)]
    choice: Mutex<Option<mpsc::Sender<bool>>>,
}

impl AppState {
    fn file(&self, name: &str) -> PathBuf {
        self.data_dir.join(name)
    }

    /// The account file holds a refresh token and your passphrase, so it's written owner-only
    /// and kept out of exports.
    fn save_account(&self) -> Result<(), String> {
        let path = self.file("account.json");
        store::save(&path, &*self.account.lock().unwrap())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    fn save_history(&self) -> Result<(), String> {
        let history = self.history.lock().unwrap().clone();
        store::save(&self.file("history.json"), &history)
    }

    fn save_deleted(&self) -> Result<(), String> {
        let deleted = self.deleted.lock().unwrap().clone();
        store::save(&self.file("deleted.json"), &deleted)
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct PhaseEvent {
    phase: &'static str,
    message: String,
    words: usize,
    locked: bool,
    /// What got typed ("done"), or the as-said version ("choose")
    text: String,
}

fn emit_event(app: &AppHandle, event: PhaseEvent) {
    let _ = app.emit("yap://state", event);
}

fn emit_phase(app: &AppHandle, phase: &'static str, message: impl Into<String>, words: usize, locked: bool) {
    emit_event(app, PhaseEvent { phase, message: message.into(), words, locked, text: String::new() });
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn accessibility() -> bool {
    #[cfg(desktop)]
    return paste::accessibility_ok();
    #[cfg(mobile)]
    true
}

fn record(app: &AppHandle, d: &Dictation) -> Result<(), String> {
    let st = app.state::<AppState>();
    {
        let mut history = st.history.lock().unwrap();
        history.insert(0, d.clone());
        history.truncate(HISTORY_LIMIT);
    }
    st.save_history()?;
    let _ = app.emit("yap://dictation", d);
    Ok(())
}

// ---------- library ----------

/// Syncs with the library folder, if there is one. Returns true if anything here changed.
fn sync_library(app: &AppHandle) -> Result<bool, String> {
    let st = app.state::<AppState>();
    let folder = st.settings.lock().unwrap().library_folder.clone();
    if folder.is_empty() {
        return Ok(false);
    }
    let changed = {
        let mut profile = st.profile.lock().unwrap();
        let mut history = st.history.lock().unwrap();
        let mut deleted = st.deleted.lock().unwrap();
        let changed = library::sync(Path::new(&folder), &mut profile, &mut history, &mut deleted)?;
        if changed {
            store::save(&st.file("profile.json"), &*profile)?;
        }
        changed
    };
    if changed {
        st.save_history()?;
        st.save_deleted()?;
        let _ = app.emit("yap://library", ());
    }
    Ok(changed)
}

/// What Settings shows about cloud sync. Never includes the passphrase or the token.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatus {
    /// This computer is registered with Firebase, one way or the other.
    registered: bool,
    /// True when it registered without anyone signing in.
    anonymous: bool,
    /// Empty unless signed in with Google.
    email: String,
    has_passphrase: bool,
    min_passphrase: usize,
}

fn status_of(state: &AppState) -> CloudStatus {
    let account = state.account.lock().unwrap();
    CloudStatus {
        registered: account.registered(),
        anonymous: account.anonymous,
        email: account.email.clone(),
        has_passphrase: !account.passphrase.is_empty(),
        min_passphrase: cloud::MIN_PASSPHRASE,
    }
}

#[tauri::command]
fn cloud_status(state: State<'_, AppState>) -> CloudStatus {
    status_of(&state)
}

/// The passphrase encrypts the library either way, and on the anonymous path it also picks
/// the vault, so it's checked before it's kept.
#[tauri::command]
fn set_cloud_passphrase(state: State<'_, AppState>, passphrase: String) -> Result<CloudStatus, String> {
    cloud::check_passphrase(&passphrase)?;
    state.account.lock().unwrap().passphrase = passphrase.trim().to_string();
    state.save_account()?;
    Ok(status_of(&state))
}

/// Opens Google in the system browser and waits for it to come back. The real browser is used
/// rather than a webview so your Google password goes to Google, not through Yap.
#[tauri::command]
async fn cloud_sign_in(app: AppHandle) -> Result<CloudStatus, String> {
    let (api_key, client_id, client_secret) = {
        let st = app.state::<AppState>();
        let s = st.settings.lock().unwrap();
        (s.firebase_api_key.clone(), s.google_client_id.clone(), s.google_client_secret.clone())
    };
    let start = cloud::start_sign_in(&client_id)?;
    let (url, verifier, redirect) = (start.url.clone(), start.verifier.clone(), start.redirect.clone());
    tauri_plugin_opener::open_url(&url, None::<&str>).map_err(|e| format!("Couldn't open your browser: {e}"))?;

    let code = tauri::async_runtime::spawn_blocking(move || {
        cloud::wait_for_code(&start, std::time::Duration::from_secs(180))
    })
    .await
    .map_err(|e| e.to_string())??;

    let account = cloud::finish_sign_in(&api_key, &client_id, &client_secret, &redirect, &verifier, &code).await?;
    let st = app.state::<AppState>();
    {
        let mut held = st.account.lock().unwrap();
        // Keep the passphrase already set here: it's what decrypts what's already uploaded.
        let passphrase = held.passphrase.clone();
        *held = cloud::Account { passphrase, ..account };
    }
    st.save_account()?;
    Ok(status_of(&st))
}

/// Registers this computer without anyone signing in, so the passphrase alone is the identity.
#[tauri::command]
async fn cloud_use_passphrase_only(app: AppHandle) -> Result<CloudStatus, String> {
    let api_key = {
        let st = app.state::<AppState>();
        let key = st.settings.lock().unwrap().firebase_api_key.clone();
        key
    };
    if api_key.trim().is_empty() {
        return Err("Add your Firebase project and API key in Settings first.".into());
    }
    let fresh = cloud::register_anonymously(&api_key).await?;
    let st = app.state::<AppState>();
    {
        let mut held = st.account.lock().unwrap();
        let passphrase = held.passphrase.clone();
        *held = cloud::Account { passphrase, ..fresh };
    }
    st.save_account()?;
    Ok(status_of(&st))
}

/// Forgets this computer's registration and passphrase. Nothing local is deleted, and what's
/// in the cloud stays for whatever still has the passphrase or the account.
#[tauri::command]
fn cloud_forget(state: State<'_, AppState>) -> Result<CloudStatus, String> {
    *state.account.lock().unwrap() = cloud::Account::default();
    state.save_account()?;
    Ok(status_of(&state))
}

#[tauri::command]
async fn cloud_sync_now(app: AppHandle) -> Result<String, String> {
    let result = sync_cloud(&app).await?;
    Ok(match (result.changed, result.uploaded) {
        (true, _) => "Synced. This computer picked up changes from your other ones.".into(),
        (false, true) => "Synced. Your library is now in the cloud.".into(),
        (false, false) => "Already up to date.".into(),
    })
}

/// Syncs the library with Firestore, whichever way this computer is registered.
async fn sync_cloud(app: &AppHandle) -> Result<cloud::Synced, String> {
    let quiet = cloud::Synced { changed: false, uploaded: false };
    let st = app.state::<AppState>();
    let (on, project, api_key) = {
        let s = st.settings.lock().unwrap();
        (s.cloud_sync, s.firebase_project_id.clone(), s.firebase_api_key.clone())
    };
    let account = st.account.lock().unwrap().clone();
    if !on || !account.registered() || account.passphrase.is_empty() {
        return Ok(quiet);
    }

    // Work on copies so no lock is held across the network calls.
    let (mut profile, mut history, mut deleted) = (
        st.profile.lock().unwrap().clone(),
        st.history.lock().unwrap().clone(),
        st.deleted.lock().unwrap().clone(),
    );
    let result = cloud::sync(&project, &api_key, &account, &mut profile, &mut history, &mut deleted).await?;
    if result.changed {
        *st.profile.lock().unwrap() = profile;
        *st.history.lock().unwrap() = history;
        *st.deleted.lock().unwrap() = deleted;
        store::save(&st.file("profile.json"), &*st.profile.lock().unwrap())?;
        st.save_history()?;
        st.save_deleted()?;
        let _ = app.emit("yap://library", ());
    }
    Ok(result)
}

fn library_loop(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(8));
        if let Err(e) = sync_library(&app) {
            eprintln!("yap: library sync: {e}");
        }
    });
}

// ---------- the dictation loop (desktop) ----------

#[cfg(desktop)]
pub(crate) fn send(app: &AppHandle, input: Input) {
    let _ = app.state::<AppState>().input.lock().unwrap().send(input);
}

#[cfg(desktop)]
mod hotkey {
    use super::{send, Input};
    use std::time::Instant;
    use tauri::{plugin::TauriPlugin, AppHandle, Wry};
    use tauri_plugin_global_shortcut::{Builder, Code, GlobalShortcutExt, Shortcut, ShortcutState};

    fn escape() -> Shortcut {
        Shortcut::new(None, Code::Escape)
    }

    /// The plugin holds its own lock while calling this handler, so the handler only
    /// hands the key off. (Registering shortcuts from in here freezes the app.)
    pub fn plugin() -> TauriPlugin<Wry> {
        Builder::new()
            .with_handler(|app, shortcut, event| {
                let pressed = matches!(event.state(), ShortcutState::Pressed);
                let input = if *shortcut == escape() {
                    if !pressed {
                        return;
                    }
                    Input::Cancel
                } else if pressed {
                    Input::Down { at: Instant::now(), tap_locks: true }
                } else {
                    Input::Up { at: Instant::now() }
                };
                send(app, input);
            })
            .build()
    }

    pub fn bind(app: &AppHandle, accel: &str) -> Result<(), String> {
        app.global_shortcut()
            .register(accel)
            .map_err(|e| format!("Couldn't use \"{accel}\" as a shortcut: {e}"))
    }

    pub fn unbind(app: &AppHandle, accel: &str) {
        let _ = app.global_shortcut().unregister(accel);
    }

    /// Esc cancels, but only while Yap is listening or asking, so it isn't stolen from other
    /// apps. Never call from the shortcut handler.
    pub fn escape_active(app: &AppHandle, on: bool) {
        let gs = app.global_shortcut();
        if on {
            let _ = gs.register(escape());
        } else {
            let _ = gs.unregister(escape());
        }
    }
}

#[cfg(desktop)]
fn dictation_loop(app: AppHandle, inputs: mpsc::Receiver<Input>) {
    let mut last_tap: Option<Instant> = None;
    for input in inputs {
        let phase = *app.state::<AppState>().phase.lock().unwrap();
        match (input, phase) {
            // While Yap offers the tidier version: the talk key says yes, Esc says no.
            (Input::Tap { .. } | Input::Down { .. }, Phase::Choosing) => choose(&app, true),
            (Input::Cancel, Phase::Choosing) => choose(&app, false),
            (Input::Down { at, tap_locks }, Phase::Idle) => start_listening(&app, at, false, true, tap_locks),
            (Input::Down { .. }, Phase::Listening { locked: true, .. }) => finish(&app),
            (Input::Up { at }, Phase::Listening { since, locked: false, tap_locks, .. }) => {
                if tap_locks && at.duration_since(since) < Duration::from_millis(350) {
                    go_hands_free(&app);
                } else {
                    finish(&app);
                }
            }
            (Input::Tap { .. }, Phase::Listening { locked: true, .. }) => finish(&app),
            (Input::Tap { at }, Phase::Idle) => {
                // Double-tap the talk key for hands-free.
                if last_tap.is_some_and(|t| at.duration_since(t) < Duration::from_millis(450)) {
                    last_tap = None;
                    start_listening(&app, at, true, true, false);
                } else {
                    last_tap = Some(at);
                }
            }
            (Input::Toggle, Phase::Idle) => start_listening(&app, Instant::now(), true, false, false),
            (Input::Toggle, Phase::Listening { .. }) => finish(&app),
            (Input::Cancel, Phase::Listening { .. }) => cancel(&app),
            _ => {}
        }
    }
}

#[cfg(desktop)]
fn start_listening(app: &AppHandle, since: Instant, locked: bool, paste: bool, tap_locks: bool) {
    let st = app.state::<AppState>();
    if let Err(e) = st.recorder.start() {
        emit_phase(app, "error", e, 0, false);
        return;
    }
    *st.phase.lock().unwrap() = Phase::Listening { since, locked, paste, tap_locks };
    hotkey::escape_active(app, true);
    emit_phase(app, "listening", "", 0, locked);
}

#[cfg(desktop)]
fn go_hands_free(app: &AppHandle) {
    let st = app.state::<AppState>();
    let mut phase = st.phase.lock().unwrap();
    if let Phase::Listening { since, paste, tap_locks, .. } = *phase {
        *phase = Phase::Listening { since, locked: true, paste, tap_locks };
        drop(phase);
        emit_phase(app, "listening", "", 0, true);
    }
}

#[cfg(desktop)]
fn cancel(app: &AppHandle) {
    let st = app.state::<AppState>();
    *st.phase.lock().unwrap() = Phase::Idle;
    st.recorder.stop();
    hotkey::escape_active(app, false);
    emit_phase(app, "idle", "Cancelled", 0, false);
}

#[cfg(desktop)]
fn choose(app: &AppHandle, list: bool) {
    if let Some(answer) = app.state::<AppState>().choice.lock().unwrap().take() {
        let _ = answer.send(list);
    }
}

/// Offers the tidier version in the pill and waits up to 5 s. No answer means as said.
#[cfg(desktop)]
async fn ask_list(app: &AppHandle, d: &Dictation) -> bool {
    let (answer, wait) = mpsc::channel();
    let st = app.state::<AppState>();
    *st.choice.lock().unwrap() = Some(answer);
    *st.phase.lock().unwrap() = Phase::Choosing;
    hotkey::escape_active(app, true);
    emit_event(app, PhaseEvent { phase: "choose", message: d.list.clone(), words: d.words, locked: false, text: d.text.clone() });
    let picked = tauri::async_runtime::spawn_blocking(move || wait.recv_timeout(Duration::from_secs(5)).unwrap_or(false))
        .await
        .unwrap_or(false);
    *st.choice.lock().unwrap() = None;
    *st.phase.lock().unwrap() = Phase::Working;
    hotkey::escape_active(app, false);
    picked
}

#[cfg(desktop)]
fn finish(app: &AppHandle) {
    let paste = {
        let st = app.state::<AppState>();
        let mut phase = st.phase.lock().unwrap();
        let Phase::Listening { paste, .. } = *phase else { return };
        *phase = Phase::Working;
        paste
    };
    hotkey::escape_active(app, false);
    emit_phase(app, "thinking", "Transcribing…", 0, false);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = pipeline(&app, paste).await;
        *app.state::<AppState>().phase.lock().unwrap() = Phase::Idle;
        match result {
            Ok(Some((d, msg))) => emit_event(&app, PhaseEvent { phase: "done", message: msg, words: d.words, locked: false, text: d.text.clone() }),
            Ok(None) => emit_phase(&app, "idle", "Didn't catch that", 0, false),
            Err(e) => emit_phase(&app, "error", e, 0, false),
        }
    });
}

#[cfg(desktop)]
async fn transcribe(app: &AppHandle, settings: &Settings, profile: &Profile, samples: Vec<f32>) -> Result<String, String> {
    let vocabulary = stt::vocabulary(profile);
    if settings.stt_engine == "cloud" {
        return stt::transcribe_cloud(settings, &samples, &stt::whisper_prompt(&vocabulary)).await;
    }
    let st = app.state::<AppState>();
    let (local, data_dir) = (st.stt.clone(), st.data_dir.clone());
    let (id, lang) = (settings.local_model.clone(), settings.language.clone());
    tauri::async_runtime::spawn_blocking(move || local.transcribe(&data_dir, &id, &samples, &lang, &vocabulary))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(desktop)]
async fn pipeline(app: &AppHandle, paste: bool) -> Result<Option<(Dictation, String)>, String> {
    let st = app.state::<AppState>();
    let recorder = st.recorder.clone();
    let samples = tauri::async_runtime::spawn_blocking(move || recorder.stop())
        .await
        .map_err(|e| e.to_string())?;
    let audio_secs = samples.len() as f32 / audio::RATE as f32;
    let level = audio::rms(&samples);
    if audio_secs < 0.3 || level < 0.002 {
        // Worth saying out loud: "Didn't catch that" looks the same whether the mic was
        // muted, the talk key was tapped by accident, or you really did trail off.
        eprintln!("yap: nothing to transcribe — {audio_secs:.2}s of audio, level {level:.5}");
        return Ok(None);
    }

    let settings = st.settings.lock().unwrap().clone();
    let profile = st.profile.lock().unwrap().clone();
    let started = Instant::now();
    let raw = transcribe(app, &settings, &profile, samples).await?;
    let stt_ms = started.elapsed().as_millis() as u64;
    if raw.is_empty() {
        eprintln!("yap: the model heard {audio_secs:.2}s at level {level:.5} but returned no words");
        return Ok(None);
    }
    // Length and timing only. What you said is yours; it doesn't go in a log.
    eprintln!("yap: transcribed {audio_secs:.2}s of audio in {stt_ms}ms, {} characters", raw.chars().count());

    emit_phase(app, "thinking", "Making it sound like you…", 0, false);
    let mut d = polish::run(&settings, &profile, &raw).await;
    d.audio_secs = audio_secs;
    d.stt_ms = stt_ms;

    if settings.lists == "ask" && !d.list.is_empty() && ask_list(app, &d).await {
        d.edits.push(store::Edit {
            original: "(as said)".into(),
            replacement: if d.list.contains("- ") { "bullet list" } else { "grouped by subject" }.into(),
            kind: "formatting".into(),
            applied: true,
            why: "you picked it".into(),
        });
        d.text = std::mem::take(&mut d.list);
        d.words = store::word_count(&d.text);
    }

    // Recorded before it's pasted: pasting is the step that can fail, and losing what you
    // said because it couldn't reach another app would be the worst way to fail.
    record(app, &d)?;

    let manual = if cfg!(target_os = "macos") { "On your clipboard · ⌘V to paste" } else { "On your clipboard · Ctrl+V to paste" };
    let mut msg = if !paste {
        "Done".to_string()
    } else if settings.auto_paste {
        let (handle, text, restore) = (app.clone(), d.text.clone(), settings.restore_clipboard);
        let pasted = tauri::async_runtime::spawn_blocking(move || paste::paste(&handle, &text, restore))
            .await
            .map_err(|e| e.to_string())??;
        if pasted { "Pasted" } else { manual }.to_string()
    } else {
        paste::copy(&d.text)?;
        manual.to_string()
    };
    if !d.note.is_empty() {
        msg += " · local cleanup";
    }
    Ok(Some((d, msg)))
}

/// Loads the chosen model in the background so the first dictation is instant.
#[cfg(desktop)]
fn preload(app: &AppHandle, id: &str) {
    let st = app.state::<AppState>();
    let (local, data_dir, id) = (st.stt.clone(), st.data_dir.clone(), id.to_string());
    let vocabulary = stt::vocabulary(&st.profile.lock().unwrap());
    std::thread::spawn(move || local.preload(&data_dir, &id, &vocabulary));
}

#[cfg(desktop)]
fn place_overlay(app: &AppHandle) {
    let Some(overlay) = app.get_webview_window("overlay") else { return };
    if let Ok(Some(monitor)) = overlay.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size();
        let origin = monitor.position();
        let (w, h) = (440.0 * scale, 220.0 * scale);
        let x = origin.x as f64 + (size.width as f64 - w) / 2.0;
        let y = origin.y as f64 + size.height as f64 - h - 84.0 * scale;
        let _ = overlay.set_position(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32));
    }
    // Always there, fully transparent when idle, never takes clicks or focus.
    let _ = overlay.set_ignore_cursor_events(true);
    let _ = overlay.show();
}

#[cfg(desktop)]
fn tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;
    let open = MenuItem::with_id(app, "open", "Open Yap", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Yap", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut builder = TrayIconBuilder::with_id("yap")
        .tooltip("Yap")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        });
    // A black-and-clear template image, so macOS tints it to match the menu bar.
    #[cfg(target_os = "macos")]
    {
        builder = builder
            .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?)
            .icon_as_template(true);
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

// ---------- commands ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    settings: Settings,
    profile: Profile,
    history: Vec<Dictation>,
    models: Vec<stt::ModelInfo>,
    platform: &'static str,
    accessibility: bool,
    env_key: bool,
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> Snapshot {
    Snapshot {
        settings: state.settings.lock().unwrap().clone(),
        profile: state.profile.lock().unwrap().clone(),
        history: state.history.lock().unwrap().clone(),
        models: stt::models(&state.data_dir),
        platform: std::env::consts::OS,
        accessibility: accessibility(),
        env_key: std::env::var("ANTHROPIC_API_KEY").is_ok_and(|k| !k.is_empty()),
    }
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

/// The key combo to register, if the talk key is a combo at all. A bare modifier this
/// platform can't watch (a Mac's Option key in a settings file opened on Windows) falls
/// back to the combo, so there's always some way to talk.
#[cfg(desktop)]
fn combo(settings: &Settings) -> Option<&str> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let bare = modkey::watchable(&settings.trigger);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let bare = false;
    (!bare).then_some(settings.shortcut.as_str())
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    let old = state.settings.lock().unwrap().clone();
    #[cfg(desktop)]
    {
        let (was, now) = (combo(&old), combo(&settings));
        if was != now {
            if let Some(accel) = was {
                hotkey::unbind(&app, accel);
            }
            if let Some(accel) = now {
                if let Err(e) = hotkey::bind(&app, accel) {
                    if let Some(accel) = was {
                        let _ = hotkey::bind(&app, accel);
                    }
                    return Err(e);
                }
            }
        }
        let model_changed = old.local_model != settings.local_model || old.stt_engine != settings.stt_engine;
        if settings.stt_engine == "local" && model_changed {
            preload(&app, &settings.local_model);
        }
    }
    store::save(&state.file("settings.json"), &settings)?;
    let _ = app.emit("yap://settings", &settings);
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
fn save_profile(app: AppHandle, state: State<'_, AppState>, mut profile: Profile) -> Result<(), String> {
    {
        let mut current = state.profile.lock().unwrap();
        // Only a real edit counts as newer; otherwise synced computers would ping-pong.
        profile.updated_at = current.updated_at;
        if serde_json::to_value(&*current).ok() == serde_json::to_value(&profile).ok() {
            return Ok(());
        }
        profile.updated_at = now_ms();
        store::save(&state.file("profile.json"), &profile)?;
        *current = profile;
    }
    // Models that listen for your vocabulary need to pick up the new words.
    #[cfg(desktop)]
    {
        let settings = state.settings.lock().unwrap().clone();
        if settings.stt_engine == "local" {
            preload(&app, &settings.local_model);
        }
    }
    #[cfg(mobile)]
    let _ = app;
    Ok(())
}

#[tauri::command]
fn update_dictation(state: State<'_, AppState>, item: Dictation) -> Result<(), String> {
    {
        let mut history = state.history.lock().unwrap();
        if let Some(d) = history.iter_mut().find(|d| d.id == item.id) {
            *d = item;
        }
    }
    state.save_history()
}

#[tauri::command]
fn delete_dictation(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.history.lock().unwrap().retain(|d| d.id != id);
    state.deleted.lock().unwrap().push(id);
    state.save_deleted()?;
    state.save_history()
}

#[tauri::command]
fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    let ids: Vec<String> = state.history.lock().unwrap().drain(..).map(|d| d.id).collect();
    state.deleted.lock().unwrap().extend(ids);
    state.save_deleted()?;
    state.save_history()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportSummary {
    words: usize,
    dictations: usize,
}

#[tauri::command]
fn export_library(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let lib = library::Library::new(
        &state.profile.lock().unwrap(),
        &state.history.lock().unwrap(),
        &state.deleted.lock().unwrap(),
    );
    library::write(Path::new(&path), &lib)
}

/// Adds another library to yours. Never removes anything.
#[tauri::command]
fn import_library(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<ImportSummary, String> {
    let lib = library::read(Path::new(&path))?;
    let words = {
        let mut profile = state.profile.lock().unwrap();
        let added = library::add_profile(&mut profile, &lib.profile);
        if added > 0 {
            profile.updated_at = now_ms();
            store::save(&state.file("profile.json"), &*profile)?;
        }
        added
    };
    let dictations = {
        let deleted = state.deleted.lock().unwrap().clone();
        let mut history = state.history.lock().unwrap();
        library::merge_history(&mut history, &lib.history, &deleted)
    };
    state.save_history()?;
    let _ = app.emit("yap://library", ());
    Ok(ImportSummary { words, dictations })
}

#[tauri::command]
fn set_library_folder(app: AppHandle, state: State<'_, AppState>, folder: String) -> Result<(), String> {
    let settings = {
        let mut settings = state.settings.lock().unwrap();
        settings.library_folder = folder.clone();
        store::save(&state.file("settings.json"), &*settings)?;
        settings.clone()
    };
    let _ = app.emit("yap://settings", &settings);
    if !folder.is_empty() {
        sync_library(&app)?;
    }
    Ok(())
}

#[tauri::command]
async fn download_model(app: AppHandle, id: String) -> Result<(), String> {
    let dir = app.state::<AppState>().data_dir.clone();
    stt::download(&app, &dir, &id).await?;
    #[cfg(desktop)]
    if app.state::<AppState>().settings.lock().unwrap().local_model == id {
        preload(&app, &id);
    }
    Ok(())
}

#[tauri::command]
fn delete_model(state: State<'_, AppState>, id: String) -> Result<(), String> {
    stt::delete(&state.data_dir, &id)
}

/// The "try it" box: clean up typed text without recording or saving.
#[tauri::command]
async fn polish_text(state: State<'_, AppState>, raw: String) -> Result<Dictation, String> {
    let settings = state.settings.lock().unwrap().clone();
    let profile = state.profile.lock().unwrap().clone();
    Ok(polish::run(&settings, &profile, raw.trim()).await)
}

/// The mic button inside Yap's own window: hands-free, and the result stays in Yap.
#[tauri::command]
fn toggle_dictation(app: AppHandle) {
    #[cfg(desktop)]
    send(&app, Input::Toggle);
    #[cfg(mobile)]
    let _ = app;
}

#[tauri::command]
fn request_accessibility(app: AppHandle) {
    #[cfg(desktop)]
    paste::request_accessibility(&app);
    #[cfg(mobile)]
    let _ = app;
}

#[tauri::command]
fn check_accessibility() -> bool {
    accessibility()
}

#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    #[cfg(desktop)]
    return paste::copy(&text);
    #[cfg(mobile)]
    {
        let _ = text;
        Err("Copy isn't wired up on mobile yet.".into())
    }
}

#[tauri::command]
fn export_settings(app: AppHandle) -> Result<store::Settings, String> {
    let st = app.state::<AppState>();
    let settings = st.settings.lock().unwrap().clone();
    Ok(settings)
}

#[tauri::command]
fn import_settings(app: AppHandle, state: State<'_, AppState>, settings: store::Settings) -> Result<(), String> {
    save_settings(app, state, settings)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());
    #[cfg(desktop)]
    let builder = builder.plugin(hotkey::plugin());

    builder
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let settings: Settings = store::load(&data_dir.join("settings.json"));
            let profile: Profile = store::load(&data_dir.join("profile.json"));
            let history: Vec<Dictation> = store::load(&data_dir.join("history.json"));
            let account: cloud::Account = store::load(&data_dir.join("account.json"));
            let deleted: Vec<String> = store::load(&data_dir.join("deleted.json"));
            #[cfg(desktop)]
            let (combo, engine, model) =
                (combo(&settings).map(str::to_string), settings.stt_engine.clone(), settings.local_model.clone());
            #[cfg(desktop)]
            let (input, inputs) = mpsc::channel();

            app.manage(AppState {
                data_dir,
                settings: Mutex::new(settings),
                profile: Mutex::new(profile),
                history: Mutex::new(history),
                deleted: Mutex::new(deleted),
                account: Mutex::new(account),
                phase: Mutex::new(Phase::Idle),
                #[cfg(desktop)]
                recorder: Arc::new(audio::Recorder::spawn(app.handle().clone())),
                #[cfg(desktop)]
                stt: Arc::new(stt::Local::default()),
                #[cfg(desktop)]
                input: Mutex::new(input),
                #[cfg(desktop)]
                choice: Mutex::new(None),
            });
            library_loop(app.handle().clone());

            #[cfg(desktop)]
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || dictation_loop(handle, inputs));
                let handle = app.handle();
                if let Some(accel) = combo {
                    if let Err(e) = hotkey::bind(handle, &accel) {
                        eprintln!("yap: {e}");
                    }
                }
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                modkey::watch(handle.clone());
                if engine == "local" {
                    preload(handle, &model);
                }
                place_overlay(handle);
                tray(handle)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps Yap listening in the background.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_settings,
            save_settings,
            save_profile,
            update_dictation,
            delete_dictation,
            clear_history,
            export_library,
            import_library,
            set_library_folder,
            cloud_status,
            cloud_sign_in,
            cloud_use_passphrase_only,
            cloud_forget,
            set_cloud_passphrase,
            cloud_sync_now,
            download_model,
            delete_model,
            polish_text,
            toggle_dictation,
            request_accessibility,
            check_accessibility,
            copy_text,
            export_settings,
            import_settings,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Yap")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main(app);
            }
            let _ = (app, event);
        });
}
