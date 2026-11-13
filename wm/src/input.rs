//! Input from the backend, delivered to the seat it belongs to, and the
//! actions bindings and the control socket run.

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent, InputTime,
        KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent,
    },
    input::{
        keyboard::{FilterResult, Keysym, ModifiersState},
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    utils::{Logical, Point, SERIAL_COUNTER},
};

use crate::{
    Modifier,
    layout::Direction,
    state::{LAUNCH_ENV, ScreenId, State},
};

/// linux/input-event-codes.h
pub const BTN_LEFT: u32 = 0x110;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Spawn(String),
    Close,
    Focus(Direction),
    Move(Direction),
    Workspace(usize),
    MoveToWorkspace(usize),
    /// On, off, or toggle (None), for the seat.
    Animations(Option<bool>),
    /// Nested only: send the host's input to the next seat.
    NextSeat,
    Quit,
}

/// The action bound to `sym` with `mods` held, if any. None are bound yet:
/// add them here, e.g. `(false, keysyms::KEY_Return) => Action::Spawn("kitty".into())`
/// in a `match (mods.shift, sym.raw())`. Until then, `wm msg` drives everything.
pub fn binding(modifier: Modifier, mods: &ModifiersState, _sym: Keysym) -> Option<Action> {
    if !modifier.held(mods) {
        return None;
    }
    None
}

impl State {
    /// Input from the host's devices, on `screen`, for the seat it drives.
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>, screen: ScreenId) {
        let seat = self.seats.active;
        match event {
            InputEvent::Keyboard { event, .. } => {
                let modifier = self.config.modifier;
                let pressed = event.state() == KeyState::Pressed;
                let keyboard = self.seats[seat].seat.get_keyboard().unwrap();
                let action = keyboard.input(
                    self,
                    event.key_code(),
                    event.state(),
                    SERIAL_COUNTER.next_serial(),
                    Event::time(&event),
                    |_, mods, handle| {
                        // Shift+digits give symbols, so match on the unshifted key.
                        let sym = handle.raw_latin_sym_or_raw_current_sym().unwrap_or(handle.modified_sym());
                        match binding(modifier, mods, sym) {
                            Some(action) if pressed => FilterResult::Intercept(action),
                            _ => FilterResult::Forward,
                        }
                    },
                );
                if let Some(action) = action {
                    self.run(seat, action);
                }
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let pos = event.position_transformed(self.screen_size(screen));
                self.pointer_motion(seat, pos, Event::time(&event));
            }
            InputEvent::PointerButton { event, .. } => {
                self.pointer_button(seat, event.button_code(), event.state(), Event::time(&event));
            }
            InputEvent::PointerAxis { event, .. } => {
                let mut frame = AxisFrame::new(Event::time(&event)).source(event.source());
                for axis in [Axis::Horizontal, Axis::Vertical] {
                    let discrete = event.amount_v120(axis);
                    let amount = event.amount(axis).unwrap_or_else(|| discrete.unwrap_or(0.0) * 15.0 / 120.0);
                    if amount != 0.0 {
                        frame = frame.value(axis, amount);
                        if let Some(discrete) = discrete {
                            frame = frame.v120(axis, discrete as i32);
                        }
                    } else if event.source() == AxisSource::Finger && event.amount(axis) == Some(0.0) {
                        frame = frame.stop(axis);
                    }
                }
                self.refresh_pointer(seat, Event::time(&event));
                let pointer = self.seats[seat].seat.get_pointer().unwrap();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            _ => {}
        }
    }

    pub fn pointer_motion(&mut self, seat: usize, pos: Point<f64, Logical>, time: InputTime) {
        let under = self.surface_under(seat, pos);
        let pointer = self.seats[seat].seat.get_pointer().unwrap();
        pointer.motion(self, under, &MotionEvent { location: pos, serial: SERIAL_COUNTER.next_serial(), time });
        pointer.frame(self);
    }

    /// Point `seat`'s pointer at what is under it now: the view may have
    /// scrolled, or the seat changed workspace, since it last moved.
    pub fn refresh_pointer(&mut self, seat: usize, time: InputTime) {
        let pointer = self.seats[seat].seat.get_pointer().unwrap();
        if !pointer.is_grabbed() {
            self.pointer_motion(seat, pointer.current_location(), time);
        }
    }

    pub fn pointer_button(&mut self, seat: usize, button: u32, state: ButtonState, time: InputTime) {
        if state == ButtonState::Pressed {
            self.refresh_pointer(seat, time);
        }
        let pointer = self.seats[seat].seat.get_pointer().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        if state == ButtonState::Pressed && !pointer.is_grabbed() {
            // Clicking focuses without scrolling: the window is visible already.
            let window = self.window_under(seat, pointer.current_location()).map(|(w, _)| w);
            if window.is_some() {
                self.focus(seat, window, false);
            }
        }
        pointer.button(self, &ButtonEvent { button, state, serial, time });
        pointer.frame(self);
    }

    pub fn run(&mut self, seat: usize, action: Action) {
        match action {
            Action::Spawn(command) => {
                // Its windows open on this seat's workspace (see `home_seat`).
                let spawned = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .env(LAUNCH_ENV, seat.to_string())
                    .spawn();
                match spawned {
                    // Reaped when it exits, so it leaves no zombie.
                    Ok(mut child) => drop(std::thread::spawn(move || child.wait())),
                    Err(err) => tracing::warn!("couldn't run {command}: {err}"),
                }
            }
            Action::Close => {
                if let Some(t) = self.seats[seat].focus.as_ref().and_then(|w| w.toplevel()) {
                    t.send_close();
                }
            }
            Action::Focus(dir) => self.focus_dir(seat, dir),
            Action::Move(dir) => self.move_dir(seat, dir),
            Action::Workspace(index) => self.switch_workspace(seat, index),
            Action::MoveToWorkspace(index) => self.move_to_workspace(seat, index),
            Action::Animations(on) => self.set_animations(seat, on),
            Action::NextSeat => {
                self.seats.active = (self.seats.active + 1) % self.seats.list.len();
                self.show_active_seat();
                tracing::info!("host input now drives seat{}", self.seats.active);
            }
            Action::Quit => self.loop_signal.stop(),
        }
    }
}
