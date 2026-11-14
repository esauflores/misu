//! Remote input: any seat driven through `wm msg [seat N] …` instead of
//! (or as well as) its devices, by a script, a binding, a test or an agent.
//! It goes in as hands' would, through the seat's keyboard and pointer, so
//! bindings, focus following the mouse and menus all behave the same.
//!
//! - `key ctrl+shift+t`: a combo, pressed and released (any xkb keysym
//!   name, any case; modifiers ctrl, shift, alt, super or mod).
//! - `type TEXT`: the rest of the line, as keys of the seat's layout
//!   (Shift and AltGr as needed). Characters the layout lacks are refused
//!   before anything is typed.
//! - `scroll DX DY [X Y]`: wheel notches (positive: right, down).
//! - `click [left|right|middle] [X Y]`, `double-click …`.
//! - `drag X1 Y1 X2 Y2`: press at one point, move, release at the other.
//! - `focus-window ID`: go to a window (an id from `tree`) and focus it.

use smithay::{
    backend::input::{Axis, AxisSource, ButtonState, InputTime, KeyState},
    input::{
        keyboard::{Keycode, Keysym, KeyboardSource, xkb},
        pointer::AxisFrame,
    },
};

use std::time::Duration;

use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::{
    Modifier,
    input::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, bindings::combo_key},
    state::{State, window_id},
};

/// A drag's moves, and the time between them.
const DRAG_STEPS: u32 = 10;
const DRAG_STEP: Duration = Duration::from_millis(15);

/// A pointer button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

impl Button {
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "middle" => Some(Self::Middle),
            _ => None,
        }
    }

    fn code(self) -> u32 {
        match self {
            Self::Left => BTN_LEFT,
            Self::Right => BTN_RIGHT,
            Self::Middle => BTN_MIDDLE,
        }
    }
}

/// `ctrl+shift+t` as the modifier keysyms to hold and the key: the
/// modifiers as a binding reads them (`mod` is `modifier`), the key in its
/// exact case if there is one.
pub fn parse_combo(combo: &str, modifier: Modifier) -> Result<(Vec<Keysym>, Keysym), String> {
    let (mods, lower) = combo_key(combo, modifier).map_err(|err| err.replacen("binding", "key", 1))?;
    let key = combo.rsplit('+').next().unwrap_or_default().trim();
    let sym = xkb::keysym_from_name(key, xkb::KEYSYM_NO_FLAGS);
    let sym = if sym.raw() == 0 { Keysym::new(lower) } else { sym };
    let held = [(mods.ctrl, Keysym::Control_L), (mods.shift, Keysym::Shift_L), (mods.alt, Keysym::Alt_L), (mods.logo, Keysym::Super_L)];
    Ok((held.into_iter().filter_map(|(on, sym)| on.then_some(sym)).collect(), sym))
}

/// The keysym a character types, if any: newline and tab are keys too.
fn char_keysym(c: char) -> Keysym {
    match c {
        '\n' => Keysym::Return,
        '\t' => Keysym::Tab,
        c => xkb::utf32_to_keysym(c as u32),
    }
}

impl State {
    /// Where `sym` is in `seat`'s layout: its key and the level (0 plain,
    /// 1 Shift, 2 AltGr, 3 AltGr+Shift).
    fn key_for(&mut self, seat: usize, sym: Keysym) -> Option<(Keycode, u32)> {
        let keyboard = self.seats[seat].seat.get_keyboard()?;
        keyboard.with_xkb_state(self, |ctx| {
            let xkb = ctx.xkb().lock().unwrap();
            // Safety: used only within this borrow.
            let (keymap, state) = unsafe { (xkb.keymap(), xkb.state()) };
            let layout = state.serialize_layout(xkb::STATE_LAYOUT_EFFECTIVE);
            (keymap.min_keycode().raw()..=keymap.max_keycode().raw()).map(Keycode::new).find_map(|code| {
                (0..keymap.num_levels_for_key(code, layout).min(4))
                    .find(|&level| keymap.key_get_syms_by_level(code, layout, level).contains(&sym))
                    .map(|level| (code, level))
            })
        })
    }

    /// Press `keys` in order, then release them in reverse.
    fn send_chord(&mut self, seat: usize, keys: &[Keycode]) {
        let source = KeyboardSource::new_auxiliary();
        for &code in keys {
            self.keyboard_input(seat, source, code, KeyState::Pressed, InputTime::now());
        }
        for &code in keys.iter().rev() {
            self.keyboard_input(seat, source, code, KeyState::Released, InputTime::now());
        }
    }

    /// `key COMBO`.
    pub fn send_combo(&mut self, seat: usize, combo: &str) -> Result<(), String> {
        let (mods, sym) = parse_combo(combo, self.config.modifier)?;
        let mut keys = Vec::new();
        for sym in mods.into_iter().chain([sym]) {
            let (code, _) = self.key_for(seat, sym).ok_or(format!("key {combo:?}: the layout has no {}", xkb::keysym_get_name(sym)))?;
            keys.push(code);
        }
        self.send_chord(seat, &keys);
        Ok(())
    }

    /// `type TEXT`.
    pub fn send_text(&mut self, seat: usize, text: &str) -> Result<(), String> {
        let shift = self.key_for(seat, Keysym::Shift_L).map(|k| k.0);
        let altgr = self.key_for(seat, Keysym::ISO_Level3_Shift).map(|k| k.0);
        let mut chords = Vec::new();
        let mut missing = String::new();
        for c in text.chars() {
            let found = self.key_for(seat, char_keysym(c)).and_then(|(code, level)| {
                let mut keys = Vec::new();
                if level & 2 != 0 {
                    keys.push(altgr?);
                }
                if level & 1 != 0 {
                    keys.push(shift?);
                }
                keys.push(code);
                Some(keys)
            });
            match found {
                Some(keys) => chords.push(keys),
                None if !missing.contains(c) => missing.push(c),
                None => {}
            }
        }
        if !missing.is_empty() {
            return Err(format!("the keyboard layout can't type {missing:?}"));
        }
        for keys in chords {
            self.send_chord(seat, &keys);
        }
        Ok(())
    }

    /// `click`/`double-click`: `count` clicks of `button`, after moving to
    /// `at` if given.
    pub fn send_click(&mut self, seat: usize, button: Button, at: Option<(f64, f64)>, count: u32) {
        if let Some((x, y)) = at {
            self.pointer_moved(seat, (x, y).into(), InputTime::now());
        }
        for _ in 0..count {
            self.pointer_button(seat, button.code(), ButtonState::Pressed, InputTime::now());
            self.pointer_button(seat, button.code(), ButtonState::Released, InputTime::now());
        }
    }

    /// `drag X1 Y1 X2 Y2`: press, move there in steps a few ms apart (an
    /// app sees motion that arrives with the release as no drag at all),
    /// release; then `done` with the outcome. wm goes on meanwhile.
    pub fn send_drag(&mut self, seat: usize, from: (f64, f64), to: (f64, f64), done: impl FnOnce(Result<(), String>) + 'static) {
        self.pointer_moved(seat, from.into(), InputTime::now());
        self.pointer_button(seat, BTN_LEFT, ButtonState::Pressed, InputTime::now());
        let mut step = 0;
        let mut done = Some(done);
        let timer = Timer::from_duration(DRAG_STEP);
        let inserted = self.loop_handle.insert_source(timer, move |_, _, state: &mut State| {
            step += 1;
            // Let go if the session locked meanwhile (the grab ended).
            let locked = state.is_locked();
            if !locked {
                let t = f64::from(step) / f64::from(DRAG_STEPS);
                let at = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
                state.pointer_moved(seat, at.into(), InputTime::now());
            }
            if step < DRAG_STEPS && !locked {
                return TimeoutAction::ToDuration(DRAG_STEP);
            }
            state.pointer_button(seat, BTN_LEFT, ButtonState::Released, InputTime::now());
            if let Some(done) = done.take() {
                done(if locked { Err("the session locked mid-drag".into()) } else { Ok(()) });
            }
            TimeoutAction::Drop
        });
        if let Err(err) = inserted {
            self.pointer_button(seat, BTN_LEFT, ButtonState::Released, InputTime::now());
            tracing::warn!("drag not done: {err}");
        }
    }

    /// `scroll DX DY [X Y]`, in wheel notches.
    pub fn send_scroll(&mut self, seat: usize, notches: (f64, f64), at: Option<(f64, f64)>) {
        self.seat_active(seat);
        if let Some((x, y)) = at {
            self.pointer_moved(seat, (x, y).into(), InputTime::now());
        }
        let time = InputTime::now();
        let mut frame = AxisFrame::new(time).source(AxisSource::Wheel);
        for (axis, n) in [(Axis::Horizontal, notches.0), (Axis::Vertical, notches.1)] {
            if n != 0.0 {
                frame = frame.value(axis, n * 15.0).v120(axis, (n * 120.0) as i32);
            }
        }
        self.pointer_axis(seat, frame, time);
    }

    /// `focus-window ID`.
    pub fn focus_window_id(&mut self, seat: usize, id: u64) -> Result<(), String> {
        let window = self
            .windows()
            .find(|w| window_id(w) == id)
            .cloned()
            .ok_or(format!("no window {id}"))?;
        self.activate(seat, &window);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos() {
        let combo = |c| parse_combo(c, Modifier::Alt);
        assert_eq!(combo("ctrl+shift+t"), Ok((vec![Keysym::Control_L, Keysym::Shift_L], Keysym::t)));
        assert_eq!(combo("Return"), Ok((vec![], Keysym::Return)));
        assert_eq!(combo("super+return"), Ok((vec![Keysym::Super_L], Keysym::Return)));
        assert_eq!(combo("mod+T"), Ok((vec![Keysym::Alt_L], Keysym::T)), "mod is the configured modifier");
        assert!(combo("hyper+a").is_err());
        assert!(combo("ctrl+").is_err());
        assert!(combo("nokey").is_err());
        assert_eq!(char_keysym('\n'), Keysym::Return);
        assert_eq!(char_keysym('A'), Keysym::A);
    }
}
