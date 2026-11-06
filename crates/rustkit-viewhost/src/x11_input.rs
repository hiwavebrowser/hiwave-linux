//! X11 input, translated to the viewhost's input queue (stage S1c).
//!
//! Everything here is PURE: raw X11 numbers in, [`InputKind`] out, no display. `linux.rs` fills the numbers
//! from `XEvent`s and calls these, so the translation is unit-tested without an X server.
//!
//! Conventions, chosen to match what the page input API already speaks:
//! - positions are view-local top-left, which is what an X11 child window's event coordinates are;
//! - `PendingKey::mac_keycode` is a macOS keyCode on macOS. On X11 it carries the **Win32 virtual-key code**,
//!   the numbering `Engine::handle_text_key` documents (the field name is a misnomer here, the type is unchanged
//!   so the macOS side compiles as before);
//! - wheel: buttons 4/5 are the vertical wheel, 6/7 the horizontal one, 40 px a notch, with the sign the
//!   window loop uses (`dy > 0` is wheel-up, `dy < 0` moves the page down).

use crate::input_queue::{InputKind, PendingClick, PendingKey, PendingScroll, PointerInput};
use std::time::{Duration, Instant};

/// X11 core modifier masks (`state` of key and button events).
pub const SHIFT_MASK: u32 = 1 << 0;
pub const CONTROL_MASK: u32 = 1 << 2;
/// Mod1 is Alt on every layout in common use.
pub const ALT_MASK: u32 = 1 << 3;

/// Pixels a wheel notch scrolls.
pub const WHEEL_NOTCH_PX: f64 = 40.0;

/// Two presses within this time and [`MULTI_CLICK_SLOP_PX`] count as a double click.
pub const MULTI_CLICK_WINDOW: Duration = Duration::from_millis(400);
pub const MULTI_CLICK_SLOP_PX: f64 = 4.0;

/// Counts consecutive presses of the primary button into 1, 2, 3 (X11 reports no click count).
#[derive(Debug, Default)]
pub struct ClickTracker {
    last: Option<(Instant, f64, f64, u32)>,
}

impl ClickTracker {
    /// The click count for a primary press at (`x`, `y`) at time `at`. Wraps after a triple click.
    pub fn press(&mut self, at: Instant, x: f64, y: f64) -> u32 {
        let clicks = match self.last {
            Some((t, lx, ly, n))
                if at.saturating_duration_since(t) <= MULTI_CLICK_WINDOW
                    && (x - lx).abs() <= MULTI_CLICK_SLOP_PX
                    && (y - ly).abs() <= MULTI_CLICK_SLOP_PX =>
            {
                if n >= 3 { 1 } else { n + 1 }
            }
            _ => 1,
        };
        self.last = Some((at, x, y, clicks));
        clicks
    }
}

/// A button press or release. Buttons 4 to 7 are the wheel (a press is one notch, a release is nothing);
/// 1 is primary, 3 is secondary; 2 (middle) and 8+ (thumb buttons) are not delivered.
pub fn button_input(button: u32, down: bool, x: f64, y: f64, state: u32, clicks: u32) -> Option<InputKind> {
    let shift = state & SHIFT_MASK != 0;
    match button {
        1 => Some(InputKind::Pointer(PendingClick { x, y, down, input: PointerInput::Button, clicks, shift })),
        3 => Some(InputKind::Pointer(PendingClick { x, y, down, input: PointerInput::Secondary, clicks: 1, shift })),
        4..=7 if down => {
            let (dx, dy) = match button {
                4 => (0.0, WHEEL_NOTCH_PX),
                5 => (0.0, -WHEEL_NOTCH_PX),
                6 => (-WHEEL_NOTCH_PX, 0.0),
                _ => (WHEEL_NOTCH_PX, 0.0),
            };
            Some(InputKind::Wheel(PendingScroll { x, y, dx, dy }))
        }
        _ => None,
    }
}

/// The pointer moved over the view.
pub fn motion_input(x: f64, y: f64, state: u32) -> InputKind {
    InputKind::Pointer(PendingClick {
        x,
        y,
        down: false,
        input: PointerInput::Move,
        clicks: 1,
        shift: state & SHIFT_MASK != 0,
    })
}

/// The pointer left the view.
pub fn leave_input() -> InputKind {
    InputKind::Pointer(PendingClick { x: 0.0, y: 0.0, down: false, input: PointerInput::Leave, clicks: 1, shift: false })
}

// X11 keysyms used below (X11/keysymdef.h).
const XK_BACKSPACE: u32 = 0xff08;
const XK_TAB: u32 = 0xff09;
const XK_ISO_LEFT_TAB: u32 = 0xfe20;
const XK_RETURN: u32 = 0xff0d;
const XK_ESCAPE: u32 = 0xff1b;
const XK_HOME: u32 = 0xff50;
const XK_LEFT: u32 = 0xff51;
const XK_UP: u32 = 0xff52;
const XK_RIGHT: u32 = 0xff53;
const XK_DOWN: u32 = 0xff54;
const XK_PAGE_UP: u32 = 0xff55;
const XK_PAGE_DOWN: u32 = 0xff56;
const XK_END: u32 = 0xff57;
const XK_INSERT: u32 = 0xff63;
const XK_DELETE: u32 = 0xffff;
const XK_KP_ENTER: u32 = 0xff8d;
const XK_F1: u32 = 0xffbe;
const XK_F12: u32 = 0xffc9;
const XK_SPACE: u32 = 0x20;

/// The Win32 virtual-key code for a keysym, 0 when there is none the page input knows.
pub fn keysym_to_vk(keysym: u32) -> u32 {
    match keysym {
        XK_BACKSPACE => 0x08,
        XK_TAB | XK_ISO_LEFT_TAB => 0x09,
        XK_RETURN | XK_KP_ENTER => 0x0D,
        XK_ESCAPE => 0x1B,
        XK_SPACE => 0x20,
        XK_PAGE_UP => 0x21,
        XK_PAGE_DOWN => 0x22,
        XK_END => 0x23,
        XK_HOME => 0x24,
        XK_LEFT => 0x25,
        XK_UP => 0x26,
        XK_RIGHT => 0x27,
        XK_DOWN => 0x28,
        XK_INSERT => 0x2D,
        XK_DELETE => 0x2E,
        // '0'..'9' are VK 0x30..0x39, 'a'..'z' / 'A'..'Z' are VK 0x41..0x5A.
        0x30..=0x39 => keysym,
        0x61..=0x7a => keysym - 0x20,
        0x41..=0x5a => keysym,
        XK_F1..=XK_F12 => 0x70 + (keysym - XK_F1),
        // US-layout punctuation, the OEM keys.
        0x3b | 0x3a => 0xBA, // ; :
        0x3d | 0x2b => 0xBB, // = +
        0x2c | 0x3c => 0xBC, // , <
        0x2d | 0x5f => 0xBD, // - _
        0x2e | 0x3e => 0xBE, // . >
        0x2f | 0x3f => 0xBF, // / ?
        0x60 | 0x7e => 0xC0, // ` ~
        0x5b | 0x7b => 0xDB, // [ {
        0x5c | 0x7c => 0xDC, // \ |
        0x5d | 0x7d => 0xDD, // ] }
        0x27 | 0x22 => 0xDE, // ' "
        _ => 0,
    }
}

/// A key press or release. `text` is what `XLookupString` produced; it is dropped when it is a control
/// character or when Control or Alt is held (a chord is a command, not typing).
pub fn key_input(keysym: u32, text: &str, state: u32, up: bool) -> InputKind {
    let ctrl = state & CONTROL_MASK != 0;
    let alt = state & ALT_MASK != 0;
    let typed = !ctrl && !alt && !text.is_empty() && text.chars().all(|c| !c.is_control());
    InputKind::Key(PendingKey {
        text: if typed { text.to_string() } else { String::new() },
        mac_keycode: keysym_to_vk(keysym) as u16,
        up,
        ctrl,
        cmd: false,
        shift: state & SHIFT_MASK != 0,
        alt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(k: InputKind) -> PendingClick {
        match k {
            InputKind::Pointer(p) => p,
            other => panic!("not a pointer: {other:?}"),
        }
    }

    #[test]
    fn primary_press_and_release_are_button_events() {
        let p = pointer(button_input(1, true, 10.0, 20.0, 0, 1).unwrap());
        assert!(p.down && p.input == PointerInput::Button && (p.x, p.y) == (10.0, 20.0) && p.clicks == 1 && !p.shift);
        let r = pointer(button_input(1, false, 10.0, 20.0, SHIFT_MASK, 1).unwrap());
        assert!(!r.down && r.input == PointerInput::Button && r.shift);
    }

    #[test]
    fn button_three_is_the_secondary_button() {
        let p = pointer(button_input(3, true, 1.0, 2.0, 0, 1).unwrap());
        assert_eq!(p.input, PointerInput::Secondary);
        assert!(p.down);
    }

    #[test]
    fn wheel_buttons_make_one_notch_with_the_window_loops_sign() {
        let up = match button_input(4, true, 5.0, 6.0, 0, 1).unwrap() {
            InputKind::Wheel(w) => w,
            o => panic!("{o:?}"),
        };
        assert_eq!((up.dx, up.dy, up.x, up.y), (0.0, 40.0, 5.0, 6.0));
        let down = match button_input(5, true, 0.0, 0.0, 0, 1).unwrap() {
            InputKind::Wheel(w) => w,
            o => panic!("{o:?}"),
        };
        assert_eq!(down.dy, -40.0);
        let left = match button_input(6, true, 0.0, 0.0, 0, 1).unwrap() {
            InputKind::Wheel(w) => w,
            o => panic!("{o:?}"),
        };
        assert_eq!((left.dx, left.dy), (-40.0, 0.0));
    }

    #[test]
    fn a_wheel_release_and_unused_buttons_record_nothing() {
        assert!(button_input(4, false, 0.0, 0.0, 0, 1).is_none());
        assert!(button_input(2, true, 0.0, 0.0, 0, 1).is_none());
        assert!(button_input(9, true, 0.0, 0.0, 0, 1).is_none());
    }

    #[test]
    fn motion_and_leave() {
        let m = pointer(motion_input(3.0, 4.0, 0));
        assert_eq!((m.input, m.x, m.y), (PointerInput::Move, 3.0, 4.0));
        assert_eq!(pointer(leave_input()).input, PointerInput::Leave);
    }

    #[test]
    fn clicks_count_up_within_the_window_and_slop_then_wrap() {
        let mut t = ClickTracker::default();
        let t0 = Instant::now();
        assert_eq!(t.press(t0, 10.0, 10.0), 1);
        assert_eq!(t.press(t0 + Duration::from_millis(200), 12.0, 11.0), 2);
        assert_eq!(t.press(t0 + Duration::from_millis(350), 12.0, 11.0), 3);
        // a fourth press starts over
        assert_eq!(t.press(t0 + Duration::from_millis(500), 12.0, 11.0), 1);
    }

    #[test]
    fn a_slow_or_distant_press_is_a_new_single_click() {
        let mut t = ClickTracker::default();
        let t0 = Instant::now();
        assert_eq!(t.press(t0, 10.0, 10.0), 1);
        assert_eq!(t.press(t0 + Duration::from_millis(900), 10.0, 10.0), 1, "too slow");
        assert_eq!(t.press(t0 + Duration::from_millis(1000), 50.0, 10.0), 1, "too far");
    }

    fn key(k: InputKind) -> PendingKey {
        match k {
            InputKind::Key(k) => k,
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn letters_map_to_the_win32_virtual_keys_in_either_case() {
        assert_eq!(keysym_to_vk(0x61), 0x41); // a
        assert_eq!(keysym_to_vk(0x41), 0x41); // A
        assert_eq!(keysym_to_vk(0x7a), 0x5A); // z
        assert_eq!(keysym_to_vk(0x35), 0x35); // 5
    }

    #[test]
    fn specials_map_to_their_virtual_keys() {
        for (sym, vk) in [
            (0xff0d, 0x0D), (0xff08, 0x08), (0xff09, 0x09), (0xff1b, 0x1B), (0x20, 0x20), (0xffff, 0x2E),
            (0xff51, 0x25), (0xff52, 0x26), (0xff53, 0x27), (0xff54, 0x28),
            (0xff55, 0x21), (0xff56, 0x22), (0xff50, 0x24), (0xff57, 0x23), (0xffbe, 0x70), (0xffc9, 0x7B),
        ] {
            assert_eq!(keysym_to_vk(sym), vk, "keysym {sym:#x}");
        }
        assert_eq!(keysym_to_vk(0xfe03), 0, "an unknown keysym has no virtual key");
    }

    #[test]
    fn typed_text_is_kept_and_a_chord_is_not_typing() {
        let k = key(key_input(0x61, "a", 0, false));
        assert_eq!((k.text.as_str(), k.mac_keycode, k.ctrl, k.up), ("a", 0x41, false, false));
        let shifted = key(key_input(0x41, "A", SHIFT_MASK, false));
        assert_eq!((shifted.text.as_str(), shifted.shift), ("A", true));
        // Ctrl+A: XLookupString gives "\x01"; the page gets the chord, not text.
        let chord = key(key_input(0x61, "\u{1}", CONTROL_MASK, false));
        assert_eq!((chord.text.as_str(), chord.ctrl, chord.mac_keycode), ("", true, 0x41));
        // Alt+x: a command too.
        assert_eq!(key(key_input(0x78, "x", ALT_MASK, false)).text, "");
    }

    #[test]
    fn return_and_backspace_carry_no_text_and_a_release_is_marked() {
        assert_eq!(key(key_input(0xff0d, "\r", 0, false)).text, "");
        assert_eq!(key(key_input(0xff08, "\u{8}", 0, false)).text, "");
        assert!(key(key_input(0x61, "a", 0, true)).up);
    }
}
