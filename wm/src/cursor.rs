//! Each seat's pointer, drawn with a real cursor: the one its app asks for
//! (a surface of its own, or a shape by name through `cursor-shape`), else
//! the theme's default. The theme and size are `XCURSOR_THEME` and
//! `XCURSOR_SIZE`, as for apps.
//!
//! Seat 0 has the theme as it is; every other seat has it tinted its seat
//! colour (`colors.seats`), so pointers on one screen tell their seats
//! apart. A cursor an app draws itself (a surface) can't be tinted.

use std::{collections::HashMap, io::Read, time::Instant};

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                Kind,
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
                surface::render_elements_from_surface_tree,
            },
            gles::GlesRenderer,
        },
    },
    input::pointer::{CursorIcon, CursorImageStatus, CursorImageSurfaceData},
    utils::{Logical, Point, Scale, Transform},
    wayland::compositor::with_states,
};
use xcursor::{
    CursorTheme,
    parser::{Image, parse_xcursor},
};

use crate::{
    shot::ShotElement,
    state::{State, WorkspaceId},
};

/// A cursor frame ready to draw, and its hotspot.
type Drawn = (MemoryRenderBuffer, Point<i32, Logical>);

/// The theme's cursors, loaded once each, and drawn per tint.
pub struct Cursors {
    theme: CursorTheme,
    size: u32,
    /// A cursor's images (frames, sizes) by name; None if the theme lacks it.
    images: HashMap<String, Option<Vec<Image>>>,
    /// Ready to draw: by name, frame and tint.
    buffers: HashMap<(String, usize, [u8; 3]), Drawn>,
    start: Instant,
}

impl Default for Cursors {
    fn default() -> Self {
        let name = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "default".into());
        let size = std::env::var("XCURSOR_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(24);
        Self { theme: CursorTheme::load(&name), size, images: HashMap::new(), buffers: HashMap::new(), start: Instant::now() }
    }
}

impl Cursors {
    /// The images of the first of `names` the theme has.
    fn images(&mut self, names: &[&str]) -> Option<(String, &[Image])> {
        let name = names.iter().find(|name| {
            let theme = &self.theme;
            self.images.entry(name.to_string()).or_insert_with(|| load(theme, name)).is_some()
        })?;
        let images = self.images[*name].as_deref()?;
        Some((name.to_string(), images))
    }

    /// `icon` in `tint` (None: as the theme has it), the frame for now: the
    /// buffer and its hotspot.
    fn get(&mut self, icon: CursorIcon, tint: Option<[f32; 4]>) -> Option<Drawn> {
        let millis = self.start.elapsed().as_millis() as u32;
        let size = self.size;
        let mut names = vec![icon.name()];
        names.extend(icon.alt_names());
        names.push("default");
        let (name, images) = self.images(&names)?;
        let (index, image) = frame(millis, size, images);
        let image = image.clone();
        let rgb = tint.map_or([255; 3], |c| [c[0], c[1], c[2]].map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8));
        let key = (name, index, rgb);
        let entry = self.buffers.entry(key).or_insert_with(|| {
            let mut pixels = image.pixels_rgba.clone();
            if tint.is_some() {
                // White becomes the colour; the black outline stays.
                for px in pixels.as_chunks_mut::<4>().0 {
                    for (channel, tint) in px.iter_mut().zip(rgb) {
                        *channel = (*channel as u16 * tint as u16 / 255) as u8;
                    }
                }
            }
            let buffer = MemoryRenderBuffer::from_slice(
                &pixels,
                Fourcc::Abgr8888,
                (image.width as i32, image.height as i32),
                1,
                Transform::Normal,
                None,
            );
            (buffer, (image.xhot as i32, image.yhot as i32).into())
        });
        Some(entry.clone())
    }
}

fn load(theme: &CursorTheme, name: &str) -> Option<Vec<Image>> {
    let path = theme.load_icon(name)?;
    let mut data = Vec::new();
    std::fs::File::open(path).ok()?.read_to_end(&mut data).ok()?;
    parse_xcursor(&data).filter(|images| !images.is_empty())
}

/// The frame of an animated cursor (one for a still one) at `millis`, of
/// the images nearest `size`, and its index.
fn frame(millis: u32, size: u32, images: &[Image]) -> (usize, &Image) {
    let nearest = images.iter().min_by_key(|i| (size as i32 - i.size as i32).abs()).unwrap();
    let frames: Vec<(usize, &Image)> =
        images.iter().enumerate().filter(|(_, i)| i.width == nearest.width && i.height == nearest.height).collect();
    let total: u32 = frames.iter().map(|(_, i)| i.delay).sum();
    if total == 0 {
        return frames[0];
    }
    let mut millis = millis % total;
    for &(index, image) in &frames {
        if millis < image.delay {
            return (index, image);
        }
        millis -= image.delay;
    }
    frames[0]
}

impl State {
    /// The pointers of the seats on workspace `shown`, as seat `viewer`
    /// sees it: each over what it points at in the viewer's view.
    pub fn pointers_of(&mut self, renderer: &mut GlesRenderer, shown: WorkspaceId, viewer: usize, now: Instant) -> Vec<ShotElement> {
        let mut elements = Vec::new();
        let ws = &self.workspaces[&shown];
        let viewer_view = ws.view(viewer, now);
        let seats: Vec<(usize, Point<f64, Logical>, CursorImageStatus)> = self
            .seats
            .list
            .iter()
            .enumerate()
            .filter(|(_, s)| s.workspace == shown)
            .map(|(i, s)| {
                let pointer = s.seat.get_pointer().unwrap().current_location();
                let x = pointer.x + ws.view(i, now) - viewer_view;
                (i, Point::from((x, pointer.y)), s.cursor.clone())
            })
            .collect();
        for (index, at, status) in seats {
            let at = at.to_i32_round::<i32>();
            match status {
                CursorImageStatus::Hidden => {}
                CursorImageStatus::Surface(surface) => {
                    let hotspot = with_states(&surface, |states| {
                        states
                            .data_map
                            .get::<CursorImageSurfaceData>()
                            .map(|d| d.lock().unwrap().hotspot)
                            .unwrap_or_default()
                    });
                    let loc = (at - hotspot).to_physical(1);
                    elements.extend(
                        render_elements_from_surface_tree(renderer, &surface, loc, Scale::from(1.0), 1.0, Kind::Cursor)
                            .into_iter()
                            .map(ShotElement::Surface),
                    );
                }
                CursorImageStatus::Named(icon) => {
                    let tint = (index > 0).then(|| self.config.colors.seat(index));
                    if let Some((buffer, hotspot)) = self.cursors.get(icon, tint) {
                        let loc = (at - hotspot).to_f64().to_physical(1.0);
                        if let Ok(element) =
                            MemoryRenderBufferRenderElement::from_buffer(renderer, loc, &buffer, None, None, None, Kind::Cursor)
                        {
                            elements.push(ShotElement::Cursor(element));
                        }
                    }
                }
            }
        }
        elements
    }
}
