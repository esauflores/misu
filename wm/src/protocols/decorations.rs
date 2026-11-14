//! Server-side decorations: apps are told not to draw their own title bars
//! (`xdg-decoration`, server side always), and wm draws a border round each
//! window instead, `BORDER` px wide in the gap around it, in the colour of
//! the first seat focusing it, else the inactive colour. A full-screen
//! window has none. The colours are the config file's `colors`.
//!
//! GTK 4 apps don't speak `xdg-decoration` and keep their own; kitty,
//! Firefox, Qt and others drop theirs.

use std::sync::Mutex;

use serde_json::Value;
use smithay::{
    backend::renderer::element::{
        Kind,
        solid::{SolidColorBuffer, SolidColorRenderElement},
    },
    desktop::Window,
    reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
    utils::{Logical, Point, Rectangle},
    wayland::shell::xdg::{
        ToplevelSurface,
        decoration::{XdgDecorationHandler, XdgDecorationState},
    },
};

use crate::state::State;

/// The border's width, in the gap around the window.
pub const BORDER: i32 = 2;

/// The colours seats and borders are drawn in.
#[derive(Debug, Clone, PartialEq)]
pub struct Colors {
    /// Seat N's pointer and focus border: the Nth, round again past the end.
    pub seats: Vec<[f32; 4]>,
    /// The border of a window no seat focuses.
    pub inactive: [f32; 4],
}

impl Colors {
    pub fn seat(&self, index: usize) -> [f32; 4] {
        self.seats.get(index % self.seats.len().max(1)).copied().unwrap_or([1.0; 4])
    }

    /// `{"seats": ["#rrggbb", …], "inactive": "#rrggbb"}`, either key over
    /// `self`.
    pub fn merged(&self, value: &Value) -> Result<Self, String> {
        let Value::Object(map) = value else { return Err("colors must be an object".into()) };
        let mut colors = self.clone();
        for (key, value) in map {
            match (key.as_str(), value) {
                ("seats", Value::Array(list)) if !list.is_empty() => {
                    colors.seats = list.iter().map(color).collect::<Result<_, _>>()?;
                }
                ("inactive", value) => colors.inactive = color(value)?,
                _ => return Err(format!("colors: unknown or malformed key {key:?}")),
            }
        }
        Ok(colors)
    }
}

/// `"#rrggbb"` or `"#rrggbbaa"`.
fn color(value: &Value) -> Result<[f32; 4], String> {
    let text = value.as_str().ok_or("a colour is a \"#rrggbb\" string")?;
    let hex = text.strip_prefix('#').filter(|h| matches!(h.len(), 6 | 8) && h.is_ascii()).ok_or(format!("bad colour {text:?}"))?;
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map(|b| b as f32 / 255.0).map_err(|_| format!("bad colour {text:?}"));
    Ok([byte(0)?, byte(2)?, byte(4)?, if hex.len() == 8 { byte(6)? } else { 1.0 }])
}

/// A window's border pieces, kept so damage tracking sees them unchanged.
struct Border(Mutex<[SolidColorBuffer; 4]>);

impl State {
    /// `window`'s border, its geometry at `loc` on screen: four strips
    /// round it. None for a full-screen one.
    pub fn border_of(&self, window: &Window, loc: Point<i32, Logical>, fullscreen: bool) -> Vec<SolidColorRenderElement> {
        if fullscreen {
            return Vec::new();
        }
        let seat = self.seats.list.iter().position(|s| s.focus.as_ref() == Some(window));
        let color = seat.map_or(self.config.colors.inactive, |s| self.config.colors.seat(s));
        let size = window.geometry().size;
        let (x, y, w, h, b) = (loc.x, loc.y, size.w, size.h, BORDER);
        let strips = [
            Rectangle::new((x - b, y - b).into(), (w + 2 * b, b).into()),
            Rectangle::new((x - b, y + h).into(), (w + 2 * b, b).into()),
            Rectangle::new((x - b, y).into(), (b, h).into()),
            Rectangle::new((x + w, y).into(), (b, h).into()),
        ];
        let border = window.user_data().get_or_insert_threadsafe(|| Border(Mutex::new(Default::default())));
        let mut buffers = border.0.lock().unwrap();
        strips
            .iter()
            .zip(buffers.iter_mut())
            .map(|(rect, buffer): (&Rectangle<i32, Logical>, _)| {
                buffer.update(rect.size, color);
                SolidColorRenderElement::from_buffer(buffer, rect.loc.to_physical(1), 1.0, 1.0, Kind::Unspecified)
            })
            .collect()
    }
}

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        server_side(&toplevel);
    }

    /// Whatever it prefers, wm draws them.
    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: Mode) {
        server_side(&toplevel);
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        server_side(&toplevel);
    }

    fn decoration_destroyed(&mut self, _toplevel: ToplevelSurface) {}
}

fn server_side(toplevel: &ToplevelSurface) {
    toplevel.with_pending_state(|s| s.decoration_mode = Some(Mode::ServerSide));
    if toplevel.is_initial_configure_sent() {
        toplevel.send_pending_configure();
    }
}

pub fn new_state(dh: &smithay::reexports::wayland_server::DisplayHandle) -> XdgDecorationState {
    XdgDecorationState::new::<State>(dh)
}
