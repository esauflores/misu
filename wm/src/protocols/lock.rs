//! The session lock (`ext-session-lock`): a locker (swaylock, hyprlock)
//! locks every seat, the AI's too. While locked:
//!
//! - each screen shows only the locker's surface for it (a plain
//!   background until it has one) and the pointers of the seats on it;
//! - every seat's keyboard goes to the lock surface of its screen, and
//!   pointers reach nothing else (menus and grabs end on locking);
//! - only the bindings in `locked_bindings` run, and `wm msg` refuses all
//!   but `tree`, `help`, `subscribe` and `reload`;
//! - screenshots fail, screen capture sees the lock screen and window
//!   capture sees nothing.
//!
//! The lock is confirmed to the locker once a locked frame was drawn. A
//! locker that dies without unlocking leaves the session locked; another
//! locker can then take over (and unlock).

use smithay::{
    backend::renderer::{
        element::{Kind, surface::render_elements_from_surface_tree},
        gles::GlesRenderer,
    },
    desktop::utils::send_frames_surface_tree,
    output::Output,
    reexports::{
        wayland_protocols::ext::session_lock::v1::server::ext_session_lock_v1::ExtSessionLockV1,
        wayland_server::{Resource, protocol::{wl_output::WlOutput, wl_surface::WlSurface}},
    },
    utils::{Logical, Point, SERIAL_COUNTER, Scale},
    wayland::session_lock::{LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker},
};

use crate::{
    backend::shot::ShotElement,
    state::{ScreenId, State},
};

/// Locked frames to draw before telling the locker it's locked.
const FRAMES_BEFORE_CONFIRMING: u8 = 2;

pub struct Lock {
    /// The locker holding the lock (dead if it crashed: still locked).
    owner: ExtSessionLockV1,
    /// Told once a locked frame is on screen.
    confirm: Option<(SessionLocker, u8)>,
    /// The locker's surface on each screen's output.
    surfaces: Vec<(Output, LockSurface)>,
}

impl State {
    pub fn is_locked(&self) -> bool {
        self.locked.is_some()
    }

    /// The lock surface on screen `id`.
    pub fn lock_surface(&self, id: ScreenId) -> Option<&WlSurface> {
        let lock = self.locked.as_ref()?;
        let output = &self.screens.get(&id)?.output;
        lock.surfaces.iter().find(|(o, s)| o == output && s.alive()).map(|(_, s)| s.wl_surface())
    }

    /// The lock surface of `seat`'s screen, else (a screen with none: a
    /// virtual one, or one unplugged) the first screen's that has one, so
    /// every seat can type at the locker.
    pub fn seat_lock_surface(&self, seat: usize) -> Option<&WlSurface> {
        self.screen_for(seat)
            .and_then(|id| self.lock_surface(id))
            .or_else(|| self.connected.iter().find_map(|&id| self.lock_surface(id)))
    }

    /// The lock surface under `pos` on `seat`'s screen.
    pub fn lock_surface_under(&self, seat: usize, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        let surface = self.seat_lock_surface(seat)?;
        smithay::desktop::utils::under_from_surface_tree(surface, pos, (0, 0), smithay::desktop::WindowSurfaceType::ALL)
            .map(|(s, p)| (s, p.to_f64()))
    }

    /// Screen `id` locked: the locker's surface, under the seats' pointers.
    pub fn lock_elements(&mut self, renderer: &mut GlesRenderer, id: ScreenId) -> Vec<ShotElement> {
        let mut elements = self.screen_pointers(renderer, id);
        if let Some(surface) = self.lock_surface(id).cloned() {
            elements.extend(
                render_elements_from_surface_tree(renderer, &surface, (0, 0), Scale::from(1.0), 1.0, Kind::Unspecified)
                    .into_iter()
                    .map(ShotElement::Surface),
            );
        }
        elements
    }

    /// After each frame: confirm a lock whose locked frames are drawn.
    pub fn lock_frame_done(&mut self) {
        let Some(lock) = &mut self.locked else { return };
        let Some((_, left)) = &mut lock.confirm else { return };
        *left = left.saturating_sub(1);
        if *left == 0 {
            let (locker, _) = lock.confirm.take().unwrap();
            locker.lock();
            tracing::info!("session locked");
        }
    }

    /// Frame callbacks for the lock surfaces.
    pub fn lock_frame(&self, time: std::time::Duration, shown: impl Fn(Option<crate::state::ScreenId>) -> bool) {
        let Some(lock) = &self.locked else { return };
        for (output, surface) in &lock.surfaces {
            let screen = self.screens.iter().find(|(_, screen)| screen.output == *output).map(|(id, _)| *id);
            if !shown(screen) {
                continue;
            }
            send_frames_surface_tree(surface.wl_surface(), output, time, None, |_, _| Some(output.clone()));
        }
    }

    /// Every seat's input to the lock surfaces (or, unlocked, back to its
    /// windows): grabs end, keyboards and pointers move.
    fn relock_input(&mut self) {
        let now = smithay::backend::input::InputTime::now();
        for seat in 0..self.seats.list.len() {
            let data = &self.seats[seat];
            let (pointer, keyboard) = (data.seat.get_pointer().unwrap(), data.seat.get_keyboard().unwrap());
            if self.is_locked() {
                if pointer.is_grabbed() {
                    pointer.unset_grab(self, SERIAL_COUNTER.next_serial(), now);
                }
                if keyboard.is_grabbed() {
                    keyboard.unset_grab(self);
                }
                self.seats.list[seat].menu = None;
            }
            self.update_keyboard(seat);
            self.refresh_pointer(seat, now);
        }
    }

    /// The size of `output`'s screen, for its lock surface.
    fn lock_size(&self, output: &Output) -> Option<(u32, u32)> {
        let screen = self.screens.values().find(|s| &s.output == output)?;
        let size = screen.size();
        Some((size.w.max(1) as u32, size.h.max(1) as u32))
    }

    /// Screens resized: their lock surfaces too.
    pub fn resize_lock_surfaces(&self) {
        let Some(lock) = &self.locked else { return };
        for (output, surface) in &lock.surfaces {
            if let Some(size) = self.lock_size(output) {
                surface.with_pending_state(|s| s.size = Some(size.into()));
                surface.send_configure();
            }
        }
    }
}

impl SessionLockHandler for State {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        if self.locked.as_ref().is_some_and(|lock| lock.owner.is_alive()) {
            // Locked by a live locker already: this one is refused.
            tracing::info!("a second locker was refused");
            return;
        }
        let taking_over = self.locked.is_some();
        let owner = confirmation.ext_session_lock().clone();
        // Taking over a dead locker's lock: locked already, so no frame to wait for.
        let frames = if taking_over { 0 } else { FRAMES_BEFORE_CONFIRMING };
        self.locked = Some(Lock { owner, confirm: Some((confirmation, frames)), surfaces: Vec::new() });
        tracing::info!("locking the session{}", if taking_over { " (a new locker took over)" } else { "" });
        self.relock_input();
    }

    fn unlock(&mut self) {
        self.locked = None;
        tracing::info!("session unlocked");
        self.relock_input();
    }

    fn new_surface(&mut self, surface: LockSurface, wl_output: WlOutput) {
        let Some(lock) = &self.locked else { return };
        if surface.ext_session_lock() != &lock.owner {
            return;
        }
        let Some(output) = Output::from_resource(&wl_output) else { return };
        if let Some(size) = self.lock_size(&output) {
            surface.with_pending_state(|s| s.size = Some(size.into()));
        }
        surface.send_configure();
        let lock = self.locked.as_mut().unwrap();
        lock.surfaces.retain(|(o, s)| o != &output && s.alive());
        lock.surfaces.push((output, surface));
        self.relock_input();
    }
}
