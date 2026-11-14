//! Input from the backend, delivered to the seat it belongs to, and the
//! actions bindings and the control socket run.

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent, InputTime,
        KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    input::{
        keyboard::{FilterResult, Keycode, KeyboardSource, Keysym, ModifiersState, keysyms},
        pointer::{AxisFrame, ButtonEvent, CursorImageStatus, MotionEvent},
    },
    utils::{Logical, Point, Size, SERIAL_COUNTER},
    wayland::shell::wlr_layer::{KeyboardInteractivity, Layer},
};

use crate::{
    Backend,
    layout::Direction,
    state::{LAUNCH_ENV, ScreenId, State},
};

/// linux/input-event-codes.h
pub const BTN_LEFT: u32 = 0x110;
pub const BTN_RIGHT: u32 = 0x111;
pub const BTN_MIDDLE: u32 = 0x112;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Spawn(String),
    Close,
    Focus(Direction),
    /// The first (false) or last (true) column.
    FocusEnd(bool),
    /// The window the seat focused before this one.
    FocusPrevious,
    Move(Direction),
    Workspace(usize),
    /// And, with `true`, go there with it.
    MoveToWorkspace(usize, bool),
    /// On, off, or toggle (None), for the seat.
    Animations(Option<bool>),
    /// The focused window full screen: on, off, or toggle (None).
    Fullscreen(Option<bool>),
    /// Nested only: send the host's input to the next seat.
    NextSeat,
    /// Read the config file again (bindings, modifier, devices).
    Reload,
    Quit,
}

/// Keep physical-device motion inside its seat's screen, including tiny modes.
pub fn clamp_pointer(pos: Point<f64, Logical>, size: Size<i32, Logical>) -> Point<f64, Logical> {
    (pos.x.clamp(0.0, (size.w - 1).max(0) as f64), pos.y.clamp(0.0, (size.h - 1).max(0) as f64)).into()
}

/// VT switching belongs to the direct session; nested shortcuts go to clients.
pub fn virtual_terminal(backend: Backend, mods: &ModifiersState, sym: Keysym) -> Option<i32> {
    (backend == Backend::Drm && mods.ctrl && mods.alt && (keysyms::KEY_F1..=keysyms::KEY_F12).contains(&sym.raw()))
        .then(|| (sym.raw() - keysyms::KEY_F1 + 1) as i32)
}

enum KeyboardAction {
    Run(Vec<Action>),
    ChangeVt(i32),
    Suppress,
}

impl State {
    /// Input from the host's devices, on `screen`, for the seat it drives.
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>, screen: ScreenId) {
        let seat = self.seats.active;
        self.process_seat_input_event(event, seat, screen);
    }

    /// Input from a device assigned to a particular user seat.
    pub fn process_seat_input_event<I: InputBackend>(&mut self, event: InputEvent<I>, seat: usize, screen: ScreenId) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                self.keyboard_input(seat, KeyboardSource::MAIN, event.key_code(), event.state(), Event::time(&event));
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let size = self.screen_size(screen);
                let pos = clamp_pointer(event.position_transformed(size), size);
                self.pointer_moved(seat, pos, Event::time(&event));
            }
            InputEvent::PointerMotion { event, .. } => {
                let pointer = self.seats[seat].seat.get_pointer().unwrap();
                let pos = clamp_pointer(pointer.current_location() + event.delta(), self.screen_size(screen));
                self.pointer_moved(seat, pos, Event::time(&event));
            }
            InputEvent::PointerButton { event, .. } => {
                self.pointer_button(seat, event.button_code(), event.state(), Event::time(&event));
            }
            InputEvent::PointerAxis { event, .. } => {
                self.seat_active(seat);
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

    /// Returns a requested VT switch for the DRM backend to execute.
    pub fn keyboard_input(&mut self, seat: usize, source: KeyboardSource, keycode: Keycode, key_state: KeyState, time: InputTime) -> Option<i32> {
        self.seat_active(seat);
        let backend = self.config.backend;
        let keyboard = self.seats[seat].seat.get_keyboard().unwrap();
        let action = keyboard.input_from_source(source, self, keycode, key_state, SERIAL_COUNTER.next_serial(), time, |state, mods, handle| {
            // Remember intercepted presses so releases stay intercepted even
            // when Ctrl/Alt was released before the function key.
            if key_state == KeyState::Released && state.seats[seat].intercepted_keys.remove(&keycode) {
                return FilterResult::Intercept(KeyboardAction::Suppress);
            }
            // Shift+digits give symbols, so match on the unshifted key.
            let sym = handle.raw_latin_sym_or_raw_current_sym().unwrap_or(handle.modified_sym());
            if key_state == KeyState::Pressed {
                if let Some(vt) = virtual_terminal(backend, mods, sym) {
                    state.seats[seat].intercepted_keys.insert(keycode);
                    return FilterResult::Intercept(KeyboardAction::ChangeVt(vt));
                }
                let allowed = !state.is_locked() || state.config.locked_bindings.contains(&crate::bindings::pressed(mods, sym));
                if allowed && let Some(actions) = state.config.bindings.get(mods, sym) {
                    let actions = actions.to_vec();
                    state.seats[seat].intercepted_keys.insert(keycode);
                    return FilterResult::Intercept(KeyboardAction::Run(actions));
                }
            }
            FilterResult::Forward
        });
        match action {
            Some(KeyboardAction::Run(actions)) => {
                for action in actions {
                    self.run(seat, action);
                }
            }
            Some(KeyboardAction::ChangeVt(vt)) => return Some(vt),
            _ => {}
        }
        None
    }

    /// The seat's pointer moved there (a device, or `wm msg pointer`): with
    /// focus following the mouse, the window it is now on takes the seat's
    /// keyboard, without scrolling. Not while a menu or a layer surface
    /// (fuzzel) holds it.
    pub fn pointer_moved(&mut self, seat: usize, pos: Point<f64, Logical>, time: InputTime) {
        self.seat_active(seat);
        self.pointer_motion(seat, pos, time);
        let data = &self.seats[seat];
        if !self.config.input.focus_follows_mouse
            || data.layer_focus.is_some()
            || data.seat.get_pointer().unwrap().is_grabbed()
            || data.seat.get_keyboard().unwrap().is_grabbed()
        {
            return;
        }
        if let Some((window, _)) = self.window_under(seat, pos)
            && data.focus.as_ref() != Some(&window)
        {
            self.focus(seat, Some(window), false);
        }
    }

    pub fn pointer_motion(&mut self, seat: usize, pos: Point<f64, Logical>, time: InputTime) {
        let under = self.surface_under(seat, pos);
        // Over nothing (the background), the default cursor: no app sets it.
        if under.is_none() {
            self.seats.list[seat].cursor = CursorImageStatus::default_named();
        }
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
        self.seat_active(seat);
        if state == ButtonState::Pressed {
            self.refresh_pointer(seat, time);
        }
        let pointer = self.seats[seat].seat.get_pointer().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        if state == ButtonState::Pressed && !pointer.is_grabbed() {
            self.click_focus(seat, pointer.current_location());
        }
        pointer.button(self, &ButtonEvent { button, state, serial, time });
        pointer.frame(self);
    }

    /// A click focuses what it lands on, without scrolling: a window is
    /// visible already. A layer surface takes the keyboard if it wants it; a
    /// click on a window takes it back, unless a layer surface holds it
    /// (`Exclusive`, fuzzel).
    fn click_focus(&mut self, seat: usize, pos: Point<f64, Logical>) {
        let layer = |state: &Self, layers: &[Layer]| state.layer_under(seat, layers, pos).map(|(l, _, _)| l);
        let target = match layer(self, &[Layer::Overlay, Layer::Top]) {
            Some(l) => Err(l),
            None => match self.window_under(seat, pos) {
                Some((window, _)) => Ok(window),
                None => match layer(self, &[Layer::Bottom, Layer::Background]) {
                    Some(l) => Err(l),
                    None => return,
                },
            },
        };
        match target {
            Err(layer) => {
                if layer.can_receive_keyboard_focus() {
                    self.focus_layer(seat, Some(layer));
                }
            }
            Ok(window) => {
                let held = self.seats[seat]
                    .layer_focus
                    .as_ref()
                    .is_some_and(|l| l.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive);
                if !held {
                    self.seats[seat].layer_focus = None;
                }
                self.focus(seat, Some(window), false);
            }
        }
    }

    pub fn run(&mut self, seat: usize, action: Action) {
        match action {
            Action::Spawn(command) => {
                // Its windows open on this seat's workspace (see `home_seat`).
                let spawned = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .env(LAUNCH_ENV, seat.to_string())
                    .env("XDG_ACTIVATION_TOKEN", self.spawn_token(seat))
                    .spawn();
                match spawned {
                    // Reaped when it exits, so it leaves no zombie.
                    Ok(mut child) => {
                        self.note_spawn(child.id(), seat);
                        drop(std::thread::spawn(move || child.wait()));
                    }
                    Err(err) => tracing::warn!("couldn't run {command}: {err}"),
                }
            }
            Action::Close => {
                if let Some(t) = self.seats[seat].focus.as_ref().and_then(|w| w.toplevel()) {
                    t.send_close();
                }
            }
            Action::Focus(dir) => self.focus_dir(seat, dir),
            Action::FocusEnd(last) => self.focus_end(seat, last),
            Action::FocusPrevious => self.focus_previous(seat),
            Action::Move(dir) => self.move_dir(seat, dir),
            Action::Workspace(index) => self.switch_workspace(seat, index),
            Action::MoveToWorkspace(index, follow) => self.move_to_workspace(seat, index, follow),
            Action::Animations(on) => self.set_animations(seat, on),
            Action::Fullscreen(on) => {
                if let Some(window) = self.seats[seat].focus.clone() {
                    self.set_fullscreen(&window, on, Some(seat));
                }
            }
            Action::NextSeat => {
                self.seats.active = (self.seats.active + 1) % self.seats.list.len();
                self.show_active_seat();
                tracing::info!("host input now drives seat{}", self.seats.active);
            }
            Action::Reload => self.reload(),
            Action::Quit => self.loop_signal.stop(),
        }
    }
}
