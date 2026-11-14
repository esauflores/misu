//! Layer shell: bars, launchers and backgrounds (Quickshell, fuzzel) that
//! clients put on a screen's edges or over it, outside the scrolling layout.
//!
//! A layer surface belongs to a screen (`layer_map_for_output` of its
//! output), not to a workspace, so every seat on that screen sees and
//! points at it. Space it keeps for itself (`exclusive_zone`, a bar) is
//! taken off the workspaces' area (`Screen::area`).
//!
//! Keyboard focus is per seat: a layer surface that wants the keyboard
//! (`Exclusive`, fuzzel) gets the keyboard of the seat that launched it, or
//! else of the seat looking at its screen; one that takes it when clicked
//! (`OnDemand`) gets the clicking seat's. A seat's focused window keeps its
//! place underneath and gets the keyboard back when the layer surface lets go.

use std::time::Duration;

use smithay::{
    desktop::{LayerSurface, PopupKind, WindowSurfaceType, find_popup_root_surface, get_popup_toplevel_coords, layer_map_for_output},
    output::Output,
    reexports::wayland_server::protocol::{wl_output::WlOutput, wl_surface::WlSurface},
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::with_states,
        shell::{
            wlr_layer::{
                KeyboardInteractivity, Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData, WlrLayerShellHandler,
                WlrLayerShellState,
            },
            xdg::PopupSurface,
        },
    },
};

use crate::state::{ScreenId, State, is_mapped};

impl WlrLayerShellHandler for State {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    /// On the screen it asks for, else the screen of the seat that launched
    /// it, else of the seat the host drives.
    fn new_layer_surface(&mut self, surface: WlrLayerSurface, output: Option<WlOutput>, _layer: Layer, namespace: String) {
        let output = output.as_ref().and_then(Output::from_resource).or_else(|| {
            let seat = self.launcher_of(surface.wl_surface()).unwrap_or(self.seats.active);
            self.screen_for(seat).map(|id| self.screens[&id].output.clone())
        });
        let Some(output) = output else {
            surface.send_close();
            return;
        };
        if let Err(err) = layer_map_for_output(&output).map_layer(&LayerSurface::new(surface, namespace)) {
            tracing::warn!("layer surface rejected: {err}");
        }
    }

    fn new_popup(&mut self, _parent: WlrLayerSurface, popup: PopupSurface) {
        self.unconstrain_layer_popup(&popup);
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let found = self.screens.values().find_map(|s| {
            let map = layer_map_for_output(&s.output);
            map.layers().find(|l| l.layer_surface() == &surface).cloned().map(|l| (s.id, l))
        });
        if let Some((id, layer)) = found {
            layer_map_for_output(&self.screens[&id].output).unmap_layer(&layer);
            self.release_layer(&layer);
            self.resize_screen(id);
        }
    }
}

impl State {
    /// The screen whose layer surface `surface` is, and that layer surface.
    pub fn layer_of(&self, surface: &WlSurface) -> Option<(ScreenId, LayerSurface)> {
        self.screens.values().find_map(|s| {
            let map = layer_map_for_output(&s.output);
            map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).cloned().map(|l| (s.id, l))
        })
    }

    /// A layer surface committed: lay its screen's layers out again (it may
    /// have changed its size, edges or exclusive zone), answer its first
    /// commit with a configure, and settle who has its keyboard.
    pub fn layer_commit(&mut self, surface: &WlSurface) -> bool {
        let Some((id, layer)) = self.layer_of(surface) else { return false };
        let initial_sent = with_states(surface, |states| {
            states.data_map.get::<LayerSurfaceData>().is_some_and(|d| d.lock().unwrap().initial_configure_sent)
        });
        {
            let mut map = layer_map_for_output(&self.screens[&id].output);
            map.arrange();
            if !initial_sent {
                layer.layer_surface().send_configure();
            }
        }
        self.resize_screen(id);

        let wants = is_mapped(surface) && layer.can_receive_keyboard_focus();
        if !wants {
            self.release_layer(&layer);
        } else if layer.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
            && matches!(layer.layer(), Layer::Top | Layer::Overlay)
            && !self.seats.list.iter().any(|s| s.layer_focus.as_ref() == Some(&layer))
        {
            let seat = self
                .launcher_of(surface)
                .or_else(|| self.screens[&id].viewer)
                .unwrap_or(self.seats.active);
            self.focus_layer(seat, Some(layer));
        }
        true
    }

    /// Give `seat`'s keyboard to `layer`, or (None) back to its window.
    pub fn focus_layer(&mut self, seat: usize, layer: Option<LayerSurface>) {
        self.seats[seat].layer_focus = layer;
        self.update_keyboard(seat);
    }

    /// Seats whose keyboard `layer` has get it back on their windows.
    fn release_layer(&mut self, layer: &LayerSurface) {
        for seat in 0..self.seats.list.len() {
            if self.seats[seat].layer_focus.as_ref() == Some(layer) {
                self.focus_layer(seat, None);
            }
        }
    }

    /// The layer surface on `layers` of `seat`'s screen under `pos`, and
    /// the surface there (a popup of it, say) with where it is.
    pub fn layer_under(
        &self,
        seat: usize,
        layers: &[Layer],
        pos: Point<f64, Logical>,
    ) -> Option<(LayerSurface, WlSurface, Point<f64, Logical>)> {
        if self.is_locked() {
            return None;
        }
        let id = self.screen_for(seat).filter(|id| self.connected.contains(id))?;
        let map = layer_map_for_output(&self.screens[&id].output);
        let covered = self.covered(seat);
        layers.iter().filter(|&&l| !(covered && l == Layer::Top)).find_map(|&layer| {
            let found = map.layer_under(layer, pos)?;
            let loc = map.layer_geometry(found)?.loc;
            let (surface, at) = found.surface_under(pos - loc.to_f64(), WindowSurfaceType::ALL)?;
            Some((found.clone(), surface, (at + loc).to_f64()))
        })
    }

    /// Frame callbacks for every connected screen's layer surfaces, and
    /// forgetting the ones that went away.
    pub fn layers_frame(&self, time: Duration, throttle: Option<Duration>) {
        self.lock_frame(time, throttle);
        for id in &self.connected {
            let output = &self.screens[id].output;
            let mut map = layer_map_for_output(output);
            map.cleanup();
            for layer in map.layers() {
                layer.send_frame(output, time, throttle, |_, _| Some(output.clone()));
            }
        }
    }

    /// Keep a layer surface's popup on its screen.
    pub fn unconstrain_layer_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else { return };
        let Some((id, layer)) = self.layer_of(&root) else { return };
        let map = layer_map_for_output(&self.screens[&id].output);
        let Some(geometry) = map.layer_geometry(&layer) else { return };
        let mut target = Rectangle::from_size(self.screens[&id].size());
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= geometry.loc;
        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}
