//! X11 apps, through `xwayland-satellite`: it runs Xwayland and turns each
//! X11 window into an ordinary Wayland toplevel of its own connection, so
//! the layout, focus per seat, borders, dialogs (an X11 transient is an
//! xdg parent) and capture treat them like any other window.
//!
//! wm picks a free display number, starts satellite on it, and sets
//! `DISPLAY` for everything it starts. If satellite exits it is started
//! again, unless it died soon after starting (then X11 apps are off until
//! wm restarts). Off with `"xwayland": false` in the config file, or when
//! `xwayland-satellite` isn't on PATH.
//!
//! X11 has one seat: satellite's windows open on the seat that started it
//! (the one the host drives), and only one seat's input reaches X11 apps.

use std::{
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use smithay::reexports::calloop::{
    channel::{Event, channel},
    timer::{TimeoutAction, Timer},
};

use crate::state::State;

/// Ran this long, an exit is worth starting it again for.
const STEADY: Duration = Duration::from_secs(10);

/// The X display `:N` satellite serves.
pub struct Xwayland {
    pub display: String,
}

/// The first display number with neither a socket nor a lock file.
fn free_display() -> Option<u32> {
    (0..64).find(|n| {
        !Path::new(&format!("/tmp/.X11-unix/X{n}")).exists() && !Path::new(&format!("/tmp/.X{n}-lock")).exists()
    })
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

impl State {
    /// Start satellite and set `DISPLAY` for children (call while wm is
    /// still single-threaded: it sets the environment).
    pub fn start_xwayland(&mut self) {
        if !self.config.xwayland {
            return;
        }
        if !on_path("xwayland-satellite") {
            tracing::info!("X11 apps off: xwayland-satellite isn't on PATH");
            return;
        }
        let Some(n) = free_display() else {
            tracing::warn!("X11 apps off: no free X display");
            return;
        };
        let name = format!(":{n}");
        // Safety: called before any thread is started.
        unsafe { std::env::set_var("DISPLAY", &name) };
        tracing::info!("X11 apps on DISPLAY={name}, through xwayland-satellite");
        self.xwayland = Some(Xwayland { display: name.clone() });
        self.run_satellite(name);
    }

    fn run_satellite(&mut self, display: String) {
        let child = Command::new("xwayland-satellite").arg(&display).stdin(Stdio::null()).spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(err) => {
                tracing::warn!("X11 apps off: couldn't start xwayland-satellite: {err}");
                self.xwayland = None;
                return;
            }
        };
        let started = Instant::now();
        // Waited on in a thread (which also reaps it); its exit comes back
        // to the loop.
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let status = child.wait();
            let _ = tx.send(status.map(|s| s.to_string()).unwrap_or_else(|e| e.to_string()));
        });
        let inserted = self.loop_handle.insert_source(rx, move |event, _, state: &mut State| {
            let Event::Msg(status) = event else { return };
            if started.elapsed() < STEADY {
                tracing::warn!("xwayland-satellite exited ({status}) soon after starting: X11 apps off");
                state.xwayland = None;
                return;
            }
            tracing::warn!("xwayland-satellite exited ({status}); starting it again");
            let display = display.clone();
            let _ = state.loop_handle.insert_source(Timer::from_duration(Duration::from_secs(1)), move |_, _, state| {
                state.run_satellite(display.clone());
                TimeoutAction::Drop
            });
        });
        if let Err(err) = inserted {
            tracing::warn!("couldn't watch xwayland-satellite: {err}");
        }
    }
}
