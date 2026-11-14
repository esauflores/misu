//! Backends and drawing: nested in a window (`winit`) or on a VT (`udev`),
//! client dmabufs, and the scene they draw (`shot`). What both backends do
//! after a frame is here.

pub mod dmabuf;
pub mod shot;
pub mod udev;
pub mod winit;

use std::time::Duration;

use smithay::{
    backend::{drm::DrmNode, egl::EGLDevice, renderer::gles::GlesRenderer},
    output::Output,
};

use crate::state::State;

pub(crate) const BACKGROUND: [f32; 4] = [0.08, 0.08, 0.1, 1.0];

/// The render node `renderer` draws on, for dmabuf feedback.
fn render_node(renderer: &GlesRenderer) -> Option<DrmNode> {
    EGLDevice::device_for_display(renderer.egl_context().display())
        .and_then(|device| device.try_get_render_node())
        .ok()
        .flatten()
}

impl State {
    /// Frame callbacks to every live window (off-screen seats' too, on their
    /// screen's output, else `fallback`), layer surface and lock surface.
    fn send_frames(&self, fallback: &Output, throttle: Duration) {
        let elapsed = self.start_time.elapsed();
        for window in self.live_windows() {
            let output = self
                .workspace_of(window)
                .and_then(|ws| self.screen_of(ws))
                .map_or(fallback, |id| &self.screens[&id].output);
            window.send_frame(output, elapsed, Some(throttle), |_, _| Some(output.clone()));
        }
        self.layers_frame(elapsed, Some(throttle));
        self.lock_frame(elapsed, Some(throttle));
    }

    /// After drawing: spaces refreshed, closed popups gone, clients told.
    fn after_frame(&mut self) {
        for screen in self.screens.values_mut() {
            screen.space.refresh();
        }
        self.popups.cleanup();
        let _ = self.display_handle.flush_clients();
    }
}
