//! Hold-to-talk on a bare modifier key (Option or fn), which the global shortcut API
//! can't express. Polls the keyboard's modifier state, which needs no extra permission.
//!
//! Holding the key alone for a moment starts listening. If you type a key, click, scroll
//! or add another modifier first, it was part of a shortcut and Yap stays out of the way.

use crate::{send, AppState, Input};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventSourceFlagsState(state_id: i32) -> u64;
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

const HID_STATE: i32 = 1;
const LEFT_OPTION: u64 = 0x20;
const RIGHT_OPTION: u64 = 0x40;
const SHIFT: u64 = 0x2_0000;
const CONTROL: u64 = 0x4_0000;
const OPTION: u64 = 0x8_0000;
const COMMAND: u64 = 0x10_0000;
const FN: u64 = 0x80_0000;
/// Key down, left/right/other mouse down, scroll wheel.
const INTERACTIONS: [u32; 5] = [10, 1, 3, 25, 22];

/// How long the key must be held alone before Yap starts listening.
const HOLD: Duration = Duration::from_millis(180);
/// Anything shorter than this, released cleanly, counts as a tap.
const TAP: Duration = Duration::from_millis(300);

/// (is the talk key down, is it down on its own)
fn state(trigger: &str, flags: u64) -> (bool, bool) {
    let (down, extra) = match trigger {
        "option" => (flags & OPTION != 0, 0),
        "right-option" => (flags & RIGHT_OPTION != 0, LEFT_OPTION),
        "fn" => (flags & FN != 0, OPTION),
        _ => (false, 0),
    };
    (down, flags & (SHIFT | CONTROL | COMMAND | extra) == 0)
}

fn interacted_since(since: Instant) -> bool {
    let window = since.elapsed().as_secs_f64();
    INTERACTIONS
        .iter()
        .any(|&kind| unsafe { CGEventSourceSecondsSinceLastEventType(HID_STATE, kind) } < window)
}

pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut pressed: Option<Instant> = None;
        let mut active = false;
        let mut spoiled = false;
        loop {
            std::thread::sleep(Duration::from_millis(12));
            let trigger = app.state::<AppState>().settings.lock().unwrap().trigger.clone();
            let (down, alone) = state(&trigger, unsafe { CGEventSourceFlagsState(HID_STATE) });
            match (pressed, down) {
                (None, true) => {
                    pressed = Some(Instant::now());
                    active = false;
                    spoiled = !alone;
                }
                (Some(at), true) if !active && !spoiled => {
                    if !alone || interacted_since(at) {
                        spoiled = true;
                    } else if at.elapsed() >= HOLD {
                        active = true;
                        send(&app, Input::Down { at, tap_locks: false });
                    }
                }
                (Some(at), false) => {
                    if active {
                        send(&app, Input::Up { at: Instant::now() });
                    } else if !spoiled && at.elapsed() < TAP && !interacted_since(at) {
                        send(&app, Input::Tap { at: Instant::now() });
                    }
                    pressed = None;
                }
                _ => {}
            }
        }
    });
}
