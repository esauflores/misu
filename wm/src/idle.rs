//! Idle notify (`ext-idle-notify`), so an idle daemon (swayidle) can act
//! after a seat has been idle a while, and before sleep. Idle is per seat:
//! input on one seat (an agent's) doesn't make another active. Idle
//! inhibit isn't offered yet.

use smithay::wayland::idle_notify::{IdleNotifierHandler, IdleNotifierState};

use crate::state::State;

impl State {
    /// `seat` had input: it isn't idle.
    pub fn seat_active(&mut self, seat: usize) {
        let handle = self.seats[seat].seat.clone();
        self.idle.notify_activity(&handle);
    }
}

impl IdleNotifierHandler for State {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle
    }
}
