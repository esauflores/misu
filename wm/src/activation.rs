//! xdg-activation: an app brings a window forward (a link clicked in a
//! terminal raises the browser). Activation belongs to a seat: the one
//! whose input asked for it, through the token. That seat goes to the
//! window and focuses it; no other seat's focus moves, and a request with
//! no seat behind it (no recent input, an old token) does nothing.
//!
//! A token is valid when made with a serial of the seat's keyboard focus
//! on the asking app, or by wm itself for what a seat spawns
//! (`XDG_ACTIVATION_TOKEN`). It lasts 10 s, and can't activate a window
//! newer than itself: a launched app's new window opens focused on its
//! seat already, and its late self-activation would pull the seat back.

use std::time::Duration;

use smithay::{
    desktop::Window,
    input::Seat,
    reexports::wayland_server::{Resource, protocol::wl_surface::WlSurface},
    wayland::xdg_activation::{XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData},
};

use crate::state::State;

/// How long a token can activate.
const LIFETIME: Duration = Duration::from_secs(10);

/// The seat a token is for.
struct TokenSeat(usize);

/// When a window first showed (joined the layout).
pub struct MappedAt(pub std::time::Instant);

impl State {
    /// `seat` goes to `window` (its workspace, scrolled to it) and focuses it.
    pub fn activate(&mut self, seat: usize, window: &Window) {
        let Some(ws) = self.workspace_of(window) else { return };
        if self.seats[seat].workspace != ws {
            let shown = self.screen_of(ws).is_some_and(|s| self.connected.contains(&s));
            if shown {
                self.go_to(seat, ws);
            } else if self.take_workspace(seat, ws).is_err() {
                return;
            }
        }
        self.focus(seat, Some(window.clone()), true);
    }

    /// A token for what `seat` spawns, for `XDG_ACTIVATION_TOKEN`.
    pub fn spawn_token(&mut self, seat: usize) -> String {
        // Expired ones go now, so spawns don't pile them up.
        self.activation.retain_tokens(|_, data| data.timestamp.elapsed() <= LIFETIME);
        let (token, data) = self.activation.create_external_token(None);
        data.user_data.insert_if_missing(|| TokenSeat(seat));
        token.to_string()
    }

    /// The seat whose keyboard focus, on the asking client, the token's
    /// serial comes from.
    fn token_seat(&self, data: &XdgActivationTokenData) -> Option<usize> {
        let (serial, wl_seat) = data.serial.as_ref()?;
        let seat = Seat::<State>::from_resource(wl_seat)?;
        let index = self.seats.list.iter().position(|s| s.seat == seat)?;
        let keyboard = seat.get_keyboard()?;
        let focused = keyboard.current_focus()?;
        let same_client = focused.client().map(|c| c.id()) == data.client_id;
        let recent = keyboard.last_enter().is_some_and(|enter| serial.is_no_older_than(&enter));
        (same_client && recent).then_some(index)
    }
}

impl XdgActivationHandler for State {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.activation
    }

    fn token_created(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData) -> bool {
        match self.token_seat(&data) {
            Some(seat) => {
                data.user_data.insert_if_missing(|| TokenSeat(seat));
                true
            }
            None => false,
        }
    }

    fn request_activation(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData, surface: WlSurface) {
        let Some(&TokenSeat(seat)) = data.user_data.get::<TokenSeat>() else { return };
        if data.timestamp.elapsed() > LIFETIME || self.is_locked() || seat >= self.seats.list.len() {
            return;
        }
        // A window newer than the token is the launch it was for: it opened
        // on its seat, focused, already, and the seat may have moved on.
        let Some(window) = self.window_for_surface(&surface) else { return };
        if window.user_data().get::<MappedAt>().is_some_and(|m| m.0 > data.timestamp) {
            return;
        }
        {
            tracing::info!("seat{seat}: activating a window");
            self.activate(seat, &window);
        }
    }
}
