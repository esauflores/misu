//! Screenshots of a seat's own view: its workspace as it sees it (scrolled
//! its way, its pointer, focus borders, its screen's layer surfaces),
//! whether or not a screen shows it. `wm msg seat N screenshot FILE` asks;
//! the backend draws it with its next frame (`State::take_shots`), off
//! screen, and the reply waits for it.

use std::{
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::Instant,
};

use serde_json::{Value, json};
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Bind, ExportMem, Offscreen,
            damage::OutputDamageTracker,
            element::{
                AsRenderElements, memory::MemoryRenderBufferRenderElement, solid::SolidColorRenderElement,
                surface::WaylandSurfaceRenderElement,
            },
            gles::{GlesRenderer, GlesTexture},
        },
    },
    desktop::layer_map_for_output,
    utils::{Physical, Point, Rectangle, Scale, Size, Transform},
    wayland::shell::wlr_layer::Layer,
};

use crate::{
    backend::BACKGROUND,
    ipc::reply,
    state::{NOBODY, ScreenId, State, WorkspaceId},
};

smithay::backend::renderer::element::render_elements! {
    pub ShotElement<=GlesRenderer>;
    Overlay=SolidColorRenderElement,
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Cursor=MemoryRenderBufferRenderElement<GlesRenderer>,
}

/// A screenshot asked for, and who waits for it.
pub struct Shot {
    pub seat: usize,
    pub path: PathBuf,
    pub stream: UnixStream,
}

impl State {
    /// Take the screenshots asked for since the last frame, and reply.
    pub fn take_shots(&mut self, renderer: &mut GlesRenderer) {
        // Each frame: menus that closed let go of their seats.
        self.end_menus();
        for shot in std::mem::take(&mut self.shots) {
            let result = if self.is_locked() {
                Err("the session is locked".into())
            } else {
                self.shoot(renderer, shot.seat, &shot.path)
            };
            let _ = reply(&shot.stream, result);
        }
        self.take_captures(renderer);
        self.lock_frame_done();
    }

    /// Answer the screenshots asked for with `error` (no renderer now).
    pub fn fail_shots(&mut self, error: &str) {
        for shot in std::mem::take(&mut self.shots) {
            let _ = reply(&shot.stream, Err(error.into()));
        }
        self.fail_captures();
    }

    /// `seat`'s view, written to `path` as a PNG; its size in the reply.
    fn shoot(&mut self, renderer: &mut GlesRenderer, seat: usize, path: &Path) -> Result<Value, String> {
        let now = Instant::now();
        let ws = self.seats[seat].workspace;
        let screen = self.screen_of(ws);
        let size = screen.map_or_else(|| self.workspaces[&ws].size(), |id| self.screen_size(id));
        let size: Size<i32, Physical> = (size.w.max(1), size.h.max(1)).into();
        let elements = self.view_elements(renderer, ws, seat, screen, None, now);
        let pixels = render_rgba(renderer, size, &elements)?;
        write_png(path, size.w as u32, size.h as u32, &pixels)?;
        Ok(json!({ "path": path, "width": size.w, "height": size.h }))
    }

    /// What screen `id` shows: its viewer's view, or (unplugged, `wm msg
    /// screen-remove`) nothing but the background.
    pub fn screen_elements(&mut self, renderer: &mut GlesRenderer, id: ScreenId, now: Instant) -> Vec<ShotElement> {
        let Some((ws, viewer)) = self.shown_on(id) else { return Vec::new() };
        if self.is_locked() {
            return self.lock_elements(renderer, id);
        }
        let viewer = viewer.unwrap_or(NOBODY);
        let slide = self.slide(id, ws, viewer, now);
        self.view_elements(renderer, ws, viewer, Some(id), slide, now)
    }

    /// Workspace `ws` as seat `viewer` sees it, with `screen`'s layer
    /// surfaces, front to back: pointers, layers above the windows, the
    /// windows, layers below. Mid-`slide` (the workspace it leaves, and how
    /// far down `ws` still is), both workspaces' windows, a screen apart.
    pub fn view_elements(
        &mut self,
        renderer: &mut GlesRenderer,
        ws: WorkspaceId,
        viewer: usize,
        screen: Option<ScreenId>,
        slide: Option<(WorkspaceId, f64)>,
        now: Instant,
    ) -> Vec<ShotElement> {
        let mut elements = self.pointers_of(renderer, ws, viewer, now);
        // A full-screen window covering the screen hides the bar.
        let covered = self.workspaces[&ws].covered(viewer, now);
        let layers = |renderer: &mut GlesRenderer, upper: bool| -> Vec<ShotElement> {
            let Some(id) = screen else { return Vec::new() };
            let map = layer_map_for_output(&self.screens[&id].output);
            map.layers()
                .rev()
                .filter(|l| matches!(l.layer(), Layer::Overlay | Layer::Top) == upper)
                .filter(|l| !(covered && l.layer() == Layer::Top))
                .filter_map(|l| map.layer_geometry(l).map(|geo| (geo.loc, l)))
                .flat_map(|(loc, l)| l.render_elements::<WaylandSurfaceRenderElement<_>>(renderer, loc.to_physical(1), Scale::from(1.0), 1.0))
                .map(ShotElement::Surface)
                .collect()
        };
        elements.extend(layers(renderer, true));
        let below = layers(renderer, false);
        let dy = slide.map_or(0.0, |(_, dy)| dy);
        elements.extend(self.window_elements(renderer, ws, viewer, dy.round() as i32, covered, now));
        if let Some((from, dy)) = slide {
            let height = self.workspaces[&ws].size().h as f64;
            let dy = dy - height.copysign(dy);
            elements.extend(self.window_elements(renderer, from, viewer, dy.round() as i32, false, now));
        }
        elements.extend(below);
        elements
    }

    /// The windows of `ws` and their borders as `viewer` sees them, `dy`
    /// lower.
    fn window_elements(&mut self, renderer: &mut GlesRenderer, ws: WorkspaceId, viewer: usize, dy: i32, covered: bool, now: Instant) -> Vec<ShotElement> {
        let scale = Scale::from(1.0);
        let mut elements = Vec::new();
        let workspace = &self.workspaces[&ws];
        let placed: Vec<_> = workspace
            .positions(viewer, now)
            // Covered, only the full-screen window and dialogs show: the
            // neighbours' borders reach into its edges, and would show
            // through a translucent one.
            .filter(|(w, _)| !covered || workspace.is_fullscreen(w) || workspace.is_floating(w))
            .map(|(w, p)| (w.clone(), p + Point::from((0, dy))))
            .collect();
        for (window, loc) in placed.into_iter().rev() {
            // Its border, then it: a window over it (a full-screen one, a
            // dialog) covers both.
            let fullscreen = self.workspaces[&ws].is_fullscreen(&window);
            elements.extend(self.border_of(&window, loc, fullscreen).into_iter().map(ShotElement::Overlay));
            let at = (loc - window.geometry().loc).to_physical(1);
            elements.extend(
                window.render_elements::<WaylandSurfaceRenderElement<_>>(renderer, at, scale, 1.0).into_iter().map(ShotElement::Surface),
            );
        }
        elements
    }
}

/// `elements` drawn off screen at `size`: RGBA rows, top first.
pub fn render_rgba(renderer: &mut GlesRenderer, size: Size<i32, Physical>, elements: &[ShotElement]) -> Result<Vec<u8>, String> {
    fn error(what: &'static str) -> impl Fn(smithay::backend::renderer::gles::GlesError) -> String {
        move |err| format!("{what}: {err}")
    }
    let buffer_size = size.to_logical(1).to_buffer(1, Transform::Normal);
    let mut texture: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, buffer_size).map_err(error("texture"))?;
    let mut target = renderer.bind(&mut texture).map_err(error("bind"))?;
    let mut damage = OutputDamageTracker::new(size, 1.0, Transform::Normal);
    damage.render_output(renderer, &mut target, 0, elements, BACKGROUND).map_err(|err| format!("render: {err:?}"))?;
    let mapping = renderer
        .copy_framebuffer(&target, Rectangle::from_size(buffer_size), Fourcc::Abgr8888)
        .map_err(error("copy"))?;
    Ok(renderer.map_texture(&mapping).map_err(error("map"))?.to_vec())
}

/// RGBA rows, top first.
fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|err| err.to_string())?;
    writer.write_image_data(pixels).map_err(|err| err.to_string())
}

