//! Backends and drawing: nested in a window (`winit`) or on a VT (`udev`),
//! client dmabufs, and the scene they draw (`shot`). What both backends do
//! after a frame is here.

pub mod dmabuf;
pub mod shot;
pub mod udev;
pub mod winit;

use std::time::{Duration, Instant};

use smithay::{
    backend::{drm::DrmNode, egl::EGLDevice, renderer::gles::GlesRenderer},
    output::Output,
};

use crate::state::{ScreenId, State};

pub(crate) const BACKGROUND: [f32; 4] = [0.08, 0.08, 0.1, 1.0];

/// Frame pacing on real screens, for `wm msg stats`: the frames shown, the
/// refreshes between consecutive ones (1 is on time; more is a refresh
/// missed while drawing), and how long an app's commit took to reach the
/// screen. Gaps over `IDLE` refreshes are idle time, not misses.
#[derive(Debug, Default)]
pub struct FrameStats {
    since: Option<Instant>,
    frames: u64,
    gaps: [u64; 4],
    latency: Duration,
    latency_max: Duration,
    latencies: u64,
}

impl FrameStats {
    const IDLE: u32 = 8;

    /// A frame reached the screen `gap` refreshes after the last one shown
    /// there, with the first commit it showed made at `committed`.
    pub fn shown(&mut self, gap: Option<u32>, committed: Option<Instant>) {
        self.since.get_or_insert_with(Instant::now);
        self.frames += 1;
        if let Some(gap @ 1..=Self::IDLE) = gap {
            self.gaps[gap.min(4) as usize - 1] += 1;
        }
        if let Some(committed) = committed {
            let latency = committed.elapsed();
            self.latency += latency;
            self.latency_max = self.latency_max.max(latency);
            self.latencies += 1;
        }
    }

    /// The numbers so far, as JSON, starting over.
    pub fn take(&mut self) -> serde_json::Value {
        let stats = std::mem::take(self);
        let seconds = stats.since.map_or(0.0, |since| since.elapsed().as_secs_f64());
        let ms = |d: Duration| (d.as_secs_f64() * 10_000.0).round() / 10.0;
        serde_json::json!({
            "seconds": (seconds * 10.0).round() / 10.0,
            "frames": stats.frames,
            "fps": if seconds > 0.0 { (stats.frames as f64 / seconds * 10.0).round() / 10.0 } else { 0.0 },
            "gaps": {"1": stats.gaps[0], "2": stats.gaps[1], "3": stats.gaps[2], "4+": stats.gaps[3]},
            "commit_to_screen_ms": {
                "avg": if stats.latencies > 0 { ms(stats.latency / stats.latencies as u32) } else { 0.0 },
                "max": ms(stats.latency_max),
            },
        })
    }
}

/// When to draw a screen's next frame while it flips every refresh: just
/// before the refresh, leaving the time a draw takes (the slowest recent one)
/// plus a margin, so app updates that come during the refresh still make it.
/// Each frame that misses its refresh anyway leaves 1 ms more, given back
/// after 10 s without one.
#[derive(Debug, Default)]
pub struct DrawClock {
    draw: Duration,
    extra: Duration,
    clean: u32,
}

impl DrawClock {
    const MARGIN: Duration = Duration::from_micros(1500);
    const STEP: Duration = Duration::from_millis(1);
    const FORGIVE: u32 = 600;

    /// A frame took `took` to draw, from building it to the GPU finishing.
    pub fn drew(&mut self, took: Duration) {
        self.draw = took.max(self.draw.mul_f32(0.95));
    }

    /// A frame drawn for the next refresh showed on it, or `missed` it.
    pub fn shown(&mut self, missed: bool) {
        if missed {
            self.extra += Self::STEP;
            self.clean = 0;
        } else {
            self.clean += 1;
            if self.clean >= Self::FORGIVE {
                self.clean = 0;
                self.extra = self.extra.saturating_sub(Self::STEP);
            }
        }
    }

    /// The time left for drawing before a refresh, at most half of it.
    pub fn budget(&self, period: Duration) -> Duration {
        (self.draw + Self::MARGIN + self.extra).clamp(Duration::from_millis(2), period / 2)
    }
}

/// The render node `renderer` draws on, for dmabuf feedback.
fn render_node(renderer: &GlesRenderer) -> Option<DrmNode> {
    EGLDevice::device_for_display(renderer.egl_context().display())
        .and_then(|device| device.try_get_render_node())
        .ok()
        .flatten()
}

impl State {
    /// Frame callbacks for what the screens `shown` picks show, and (for
    /// `None`) the windows no screen shows, which use `fallback`.
    fn send_frames(&self, fallback: &Output, shown: impl Fn(Option<ScreenId>) -> bool) {
        let elapsed = self.start_time.elapsed();
        for window in self.live_windows() {
            let screen = self.workspace_of(window).and_then(|ws| self.screen_of(ws));
            if !shown(screen) {
                continue;
            }
            let output = screen.map_or(fallback, |id| &self.screens[&id].output);
            window.send_frame(output, elapsed, None, |_, _| Some(output.clone()));
        }
        self.layers_frame(elapsed, &shown);
        self.lock_frame(elapsed, &shown);
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
