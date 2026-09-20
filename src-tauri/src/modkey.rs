//! Hold-to-talk on a bare modifier key (Option or fn on a Mac, Ctrl or Alt on Windows),
//! which the global shortcut API can't express. Polls the keyboard's modifier state, which
//! needs no extra permission.
//!
//! Holding the key alone for a moment starts listening. If you type a key, click, scroll
//! or add another modifier first, it was part of a shortcut and Yap stays out of the way.

use crate::{send, AppState, Input};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// How long the key must be held alone before Yap starts listening.
const HOLD: Duration = Duration::from_millis(180);
/// Anything shorter than this, released cleanly, counts as a tap.
const TAP: Duration = Duration::from_millis(300);
/// How often the keyboard is sampled.
const POLL: Duration = Duration::from_millis(12);

/// Is `trigger` a bare modifier this platform can watch? Anything else is a key combo,
/// which the global shortcut API handles instead.
pub fn watchable(trigger: &str) -> bool {
    sys::keys(trigger).is_some()
}

#[cfg(target_os = "macos")]
mod sys {
    use std::time::Instant;

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

    /// (the flag that means the talk key is down, the flags that mean it was a shortcut)
    pub fn keys(trigger: &str) -> Option<(u64, u64)> {
        match trigger {
            "option" => Some((OPTION, 0)),
            "right-option" => Some((RIGHT_OPTION, LEFT_OPTION)),
            "fn" => Some((FN, OPTION)),
            _ => None,
        }
    }

    #[derive(Default)]
    pub struct Watcher;

    impl Watcher {
        pub fn new() -> Self {
            Self
        }

        /// (is the talk key down, is it down on its own)
        pub fn state(&mut self, trigger: &str) -> (bool, bool) {
            let Some((key, extra)) = keys(trigger) else {
                return (false, false);
            };
            let flags = unsafe { CGEventSourceFlagsState(HID_STATE) };
            (flags & key != 0, flags & (SHIFT | CONTROL | COMMAND | extra) == 0)
        }

        pub fn interacted_since(&mut self, since: Instant) -> bool {
            let window = since.elapsed().as_secs_f64();
            INTERACTIONS
                .iter()
                .any(|&kind| unsafe { CGEventSourceSecondsSinceLastEventType(HID_STATE, kind) } < window)
        }
    }
}

#[cfg(target_os = "windows")]
mod sys {
    use std::time::Instant;

    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(vkey: i32) -> i16;
    }

    const VK_SHIFT: i32 = 0x10;
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12;
    const VK_LWIN: i32 = 0x5B;
    const VK_RWIN: i32 = 0x5C;
    const VK_LSHIFT: i32 = 0xA0;
    const VK_RSHIFT: i32 = 0xA1;
    const VK_LCONTROL: i32 = 0xA2;
    const VK_RCONTROL: i32 = 0xA3;
    const VK_LMENU: i32 = 0xA4;
    const VK_RMENU: i32 = 0xA5;

    /// Modifiers are judged by "is the talk key down on its own", so they never count as
    /// the user typing something else.
    const MODIFIERS: [i32; 11] = [
        VK_SHIFT,
        VK_CONTROL,
        VK_MENU,
        VK_LWIN,
        VK_RWIN,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_LMENU,
        VK_RMENU,
    ];

    /// (the key that means the talk key is down, the keys that mean it was a shortcut)
    ///
    /// On a layout with AltGr, right Alt reports Ctrl as well, so "right-alt" simply never
    /// fires there and the character you were typing comes out intact.
    pub fn keys(trigger: &str) -> Option<(i32, &'static [i32])> {
        match trigger {
            "right-ctrl" => Some((VK_RCONTROL, &[VK_LCONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN])),
            "right-alt" => Some((VK_RMENU, &[VK_LMENU, VK_SHIFT, VK_CONTROL, VK_LWIN, VK_RWIN])),
            "ctrl" => Some((VK_CONTROL, &[VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN])),
            "alt" => Some((VK_MENU, &[VK_SHIFT, VK_CONTROL, VK_LWIN, VK_RWIN])),
            _ => None,
        }
    }

    fn held(vk: i32) -> bool {
        unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
    }

    /// Windows has no "when did the user last type" clock, so this remembers it instead:
    /// the low bit of GetAsyncKeyState says a key has gone down since the previous call.
    pub struct Watcher {
        last_other: Option<Instant>,
        was_down: bool,
    }

    impl Watcher {
        pub fn new() -> Self {
            Self { last_other: None, was_down: false }
        }

        /// (is the talk key down, is it down on its own)
        pub fn state(&mut self, trigger: &str) -> (bool, bool) {
            let Some((key, extra)) = keys(trigger) else {
                self.was_down = false;
                return (false, false);
            };
            let down = held(key);
            if down {
                // The first sweep of a press reports keys pressed long before it, so it only
                // serves to clear the bits; from the next one on they mean this press.
                let stale = !self.was_down;
                if self.others_pressed(key) && !stale {
                    self.last_other = Some(Instant::now());
                }
            }
            self.was_down = down;
            (down, down && !extra.iter().any(|&vk| held(vk)))
        }

        /// Did anything but the talk key and the modifiers go down since the last sweep?
        /// Every key is read even once one has hit, to clear the whole keyboard's bits.
        fn others_pressed(&self, talk: i32) -> bool {
            let mut hit = false;
            for vk in 0x01..=0xFE {
                if vk == talk || MODIFIERS.contains(&vk) {
                    continue;
                }
                if unsafe { GetAsyncKeyState(vk) } & 1 != 0 {
                    hit = true;
                }
            }
            hit
        }

        pub fn interacted_since(&mut self, since: Instant) -> bool {
            self.last_other.is_some_and(|at| at >= since)
        }
    }
}

pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut keyboard = sys::Watcher::new();
        let mut pressed: Option<Instant> = None;
        let mut active = false;
        let mut spoiled = false;
        loop {
            std::thread::sleep(POLL);
            let trigger = app.state::<AppState>().settings.lock().unwrap().trigger.clone();
            let (down, alone) = keyboard.state(&trigger);
            match (pressed, down) {
                (None, true) => {
                    pressed = Some(Instant::now());
                    active = false;
                    spoiled = !alone;
                }
                (Some(at), true) if !active && !spoiled => {
                    if !alone || keyboard.interacted_since(at) {
                        spoiled = true;
                    } else if at.elapsed() >= HOLD {
                        active = true;
                        send(&app, Input::Down { at, tap_locks: false });
                    }
                }
                (Some(at), false) => {
                    if active {
                        send(&app, Input::Up { at: Instant::now() });
                    } else if !spoiled && at.elapsed() < TAP && !keyboard.interacted_since(at) {
                        send(&app, Input::Tap { at: Instant::now() });
                    }
                    pressed = None;
                }
                _ => {}
            }
        }
    });
}
