//! Screen and window capture for other programs: `ext-image-copy-capture`
//! with screens (`ext-output-image-capture-source`) and windows
//! (`ext-foreign-toplevel-image-capture-source`, from the windows that
//! `ext-foreign-toplevel-list` lists) as sources. grim, hyprpicker and
//! xdg-desktop-portal-wlr (screen sharing) speak it.
//!
//! A screen's capture is what it shows: its viewer's view, with the
//! pointers there if the client asks for them. A window's is the window
//! alone, at its own size, wherever it is and whether or not anyone sees
//! it. A window's handle in the list has its `wm msg tree` id as its
//! identifier, so `grim -T ID` takes it.
//!
//! A frame asked for is drawn with the backend's next frame (as `shot.rs`'s
//! screenshots are), into the client's shm buffer, once there is something
//! new to show: the first frame of a session at once, later ones when what
//! they show changes. dma-buf targets aren't offered yet.

use std::{sync::Mutex, time::Instant};

use smithay::{
    backend::renderer::{
        damage::OutputDamageTracker,
        element::{AsRenderElements, Element, Kind, surface::WaylandSurfaceRenderElement},
        gles::GlesRenderer,
    },
    desktop::Window,
    output::{Output, WeakOutput},
    reexports::wayland_server::protocol::{wl_buffer::WlBuffer, wl_shm},
    utils::{Buffer as BufferCoords, Clock, Monotonic, Physical, Point, Rectangle, Scale, Size, Transform},
    wayland::{
        foreign_toplevel_list::{
            ForeignToplevelHandle, ForeignToplevelListHandler, ForeignToplevelListState, ForeignToplevelWeakHandle,
        },
        image_capture_source::{
            ImageCaptureSource, ImageCaptureSourceHandler, ImageCaptureSourceState, OutputCaptureSourceHandler,
            OutputCaptureSourceState, ToplevelCaptureSourceHandler, ToplevelCaptureSourceState,
        },
        image_copy_capture::{
            BufferConstraints, CaptureFailureReason, Frame, FrameRef, ImageCopyCaptureHandler, ImageCopyCaptureState,
            Session, SessionRef,
        },
        shm,
    },
};

use crate::{
    backend::shot::{ShotElement, render_rgba},
    state::{ScreenId, State, window_id, window_info},
};

pub struct Capture {
    _sources: ImageCaptureSourceState,
    output_sources: OutputCaptureSourceState,
    toplevel_sources: ToplevelCaptureSourceState,
    copy: ImageCopyCaptureState,
    pub toplevels: ForeignToplevelListState,
    sessions: Vec<CaptureSession>,
    clock: Clock<Monotonic>,
}

struct CaptureSession {
    session: Session,
    /// Finds what changed since the last frame sent.
    damage: OutputDamageTracker,
    size: Size<i32, Physical>,
    /// Asked for, waiting for something to show.
    pending: Option<Frame>,
    /// Nothing sent yet: the next frame goes out whatever changed.
    fresh: bool,
}

/// What a session captures, now.
enum Target {
    Screen(ScreenId),
    Window(Window),
}

/// A window's handle in the foreign toplevel list.
struct ToplevelHandle(Mutex<Option<ForeignToplevelHandle>>);

impl Capture {
    pub fn new(dh: &smithay::reexports::wayland_server::DisplayHandle) -> Self {
        Self {
            _sources: ImageCaptureSourceState::new(),
            output_sources: OutputCaptureSourceState::new::<State>(dh),
            toplevel_sources: ToplevelCaptureSourceState::new::<State>(dh),
            copy: ImageCopyCaptureState::new::<State>(dh),
            toplevels: ForeignToplevelListState::new::<State>(dh),
            sessions: Vec::new(),
            clock: Clock::new(),
        }
    }
}

impl State {
    /// List `window` for capture (it joined the layout).
    pub fn list_toplevel(&mut self, window: &Window) {
        let (app_id, title) = window_info(window);
        let handle = self.capture.toplevels.new_toplevel_with_identifier::<State>(
            title.unwrap_or_default(),
            app_id.unwrap_or_default(),
            window_id(window).to_string(),
        );
        let slot = window.user_data().get_or_insert_threadsafe(|| ToplevelHandle(Mutex::new(None)));
        *slot.0.lock().unwrap() = Some(handle);
    }

    /// Take `window` off the list (it left the layout).
    pub fn unlist_toplevel(&mut self, window: &Window) {
        let Some(slot) = window.user_data().get::<ToplevelHandle>() else { return };
        if let Some(handle) = slot.0.lock().unwrap().take() {
            self.capture.toplevels.remove_toplevel(&handle);
        }
    }

    /// Tell the list `window`'s new title or app id, if it has one.
    pub fn update_toplevel(&self, window: &Window) {
        let Some(slot) = window.user_data().get::<ToplevelHandle>() else { return };
        let slot = slot.0.lock().unwrap();
        let Some(handle) = slot.as_ref() else { return };
        let (app_id, title) = window_info(window);
        let (app_id, title) = (app_id.unwrap_or_default(), title.unwrap_or_default());
        let mut changed = false;
        if handle.title() != title {
            handle.send_title(&title);
            changed = true;
        }
        if handle.app_id() != app_id {
            handle.send_app_id(&app_id);
            changed = true;
        }
        if changed {
            handle.send_done();
        }
    }

    fn target_of(&self, source: &ImageCaptureSource) -> Option<Target> {
        if let Some(output) = source.user_data().get::<WeakOutput>() {
            let output = output.upgrade()?;
            return self.screens.values().find(|s| s.output == output).map(|s| Target::Screen(s.id));
        }
        let handle = source.user_data().get::<ForeignToplevelWeakHandle>()?.upgrade()?;
        let window = self.windows().find(|w| {
            w.user_data()
                .get::<ToplevelHandle>()
                .is_some_and(|slot| slot.0.lock().unwrap().as_ref().is_some_and(|h| h.matches(&handle)))
        })?;
        Some(Target::Window(window.clone()))
    }

    fn target_size(&self, target: &Target) -> Size<i32, Physical> {
        let size = match target {
            Target::Screen(id) => self.screen_size(*id),
            Target::Window(window) => window.geometry().size,
        };
        (size.w.max(1), size.h.max(1)).into()
    }

    /// Answer the frames waiting that have something new to show.
    pub fn take_captures(&mut self, renderer: &mut GlesRenderer) {
        let now = Instant::now();
        let mut i = 0;
        while i < self.capture.sessions.len() {
            if self.capture.sessions[i].pending.is_none() {
                i += 1;
                continue;
            }
            let source = self.capture.sessions[i].session.source();
            let Some(target) = self.target_of(&source) else {
                // Its window closed or its screen went: the session ends.
                let s = self.capture.sessions.remove(i);
                if let Some(frame) = s.pending {
                    frame.fail(CaptureFailureReason::Stopped);
                }
                s.session.stop();
                continue;
            };
            let size = self.target_size(&target);
            let s = &mut self.capture.sessions[i];
            if s.size != size {
                // Resized: the client allocates again.
                s.size = size;
                s.damage = OutputDamageTracker::new(size, 1.0, Transform::Normal);
                s.fresh = true;
                s.session.update_constraints(constraints(size));
                if let Some(frame) = s.pending.take() {
                    frame.fail(CaptureFailureReason::BufferConstraints);
                }
                i += 1;
                continue;
            }
            let pointers = s.session.draw_cursor();
            let elements = self.target_elements(renderer, &target, pointers, now);
            let s = &mut self.capture.sessions[i];
            let damage = match s.damage.damage_output(1, &elements) {
                Ok((Some(damage), _)) => damage.clone(),
                _ => Vec::new(),
            };
            if damage.is_empty() && !s.fresh {
                i += 1;
                continue;
            }
            let frame = s.pending.take().unwrap();
            let damage = if s.fresh { vec![Rectangle::from_size(size)] } else { damage };
            s.fresh = false;
            let presented = self.capture.clock.now();
            match copy_to(renderer, &frame.buffer(), size, &elements) {
                Ok(()) => {
                    let damage: Vec<Rectangle<i32, BufferCoords>> = damage
                        .into_iter()
                        .map(|r| Rectangle::new((r.loc.x, r.loc.y).into(), (r.size.w, r.size.h).into()))
                        .collect();
                    frame.success(Transform::Normal, damage, presented);
                }
                Err(reason) => frame.fail(reason),
            }
            i += 1;
        }
    }

    /// Fail the frames waiting (no renderer now); the clients ask again.
    pub fn fail_captures(&mut self) {
        for s in &mut self.capture.sessions {
            if let Some(frame) = s.pending.take() {
                frame.fail(CaptureFailureReason::Unknown);
            }
        }
    }

    fn target_elements(&mut self, renderer: &mut GlesRenderer, target: &Target, pointers: bool, now: Instant) -> Vec<ShotElement> {
        match target {
            Target::Screen(id) => {
                let elements = self.screen_elements(renderer, *id, now);
                if pointers {
                    return elements;
                }
                elements.into_iter().filter(|e| e.kind() != Kind::Cursor).collect()
            }
            Target::Window(_) if self.is_locked() => Vec::new(),
            Target::Window(window) => {
                let at = (Point::default() - window.geometry().loc).to_physical(1);
                window
                    .render_elements::<WaylandSurfaceRenderElement<_>>(renderer, at, Scale::from(1.0), 1.0)
                    .into_iter()
                    .map(ShotElement::Surface)
                    .collect()
            }
        }
    }
}

fn constraints(size: Size<i32, Physical>) -> BufferConstraints {
    BufferConstraints {
        size: (size.w, size.h).into(),
        shm: vec![wl_shm::Format::Xrgb8888, wl_shm::Format::Argb8888],
        dma: None,
    }
}

/// Draw `elements` into the client's shm `buffer`, which must be `size`.
fn copy_to(
    renderer: &mut GlesRenderer,
    buffer: &WlBuffer,
    size: Size<i32, Physical>,
    elements: &[ShotElement],
) -> Result<(), CaptureFailureReason> {
    let fits = shm::with_buffer_contents(buffer, |_, _, data| {
        data.width == size.w
            && data.height == size.h
            && data.stride >= size.w * 4
            && matches!(data.format, wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888)
    });
    if !matches!(fits, Ok(true)) {
        return Err(CaptureFailureReason::BufferConstraints);
    }
    let rgba = render_rgba(renderer, size, elements).map_err(|err| {
        tracing::warn!("capture: {err}");
        CaptureFailureReason::Unknown
    })?;
    let row = size.w as usize * 4;
    shm::with_buffer_contents_mut(buffer, |ptr, len, data| {
        let (offset, stride) = (data.offset as usize, data.stride as usize);
        if offset + stride * (size.h as usize - 1) + row > len {
            return Err(CaptureFailureReason::BufferConstraints);
        }
        // SAFETY: the range was checked against the pool's length above.
        let pool = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
        for (y, src) in rgba.chunks_exact(row).enumerate() {
            let dst = &mut pool[offset + y * stride..][..row];
            // RGBA bytes to the little-endian ARGB words shm wants: BGRA.
            for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
                *d = [s[2], s[1], s[0], s[3]];
            }
        }
        Ok(())
    })
    .map_err(|_| CaptureFailureReason::BufferConstraints)?
}

impl ForeignToplevelListHandler for State {
    fn foreign_toplevel_list_state(&mut self) -> &mut ForeignToplevelListState {
        &mut self.capture.toplevels
    }
}

impl ImageCaptureSourceHandler for State {}

impl OutputCaptureSourceHandler for State {
    fn output_capture_source_state(&mut self) -> &mut OutputCaptureSourceState {
        &mut self.capture.output_sources
    }

    fn output_source_created(&mut self, source: ImageCaptureSource, output: &Output) {
        source.user_data().insert_if_missing(|| output.downgrade());
    }
}

impl ToplevelCaptureSourceHandler for State {
    fn toplevel_capture_source_state(&mut self) -> &mut ToplevelCaptureSourceState {
        &mut self.capture.toplevel_sources
    }

    fn toplevel_source_created(&mut self, source: ImageCaptureSource, toplevel: ForeignToplevelHandle) {
        source.user_data().insert_if_missing(|| toplevel.downgrade());
    }
}

impl ImageCopyCaptureHandler for State {
    fn image_copy_capture_state(&mut self) -> &mut ImageCopyCaptureState {
        &mut self.capture.copy
    }

    fn capture_constraints(&mut self, source: &ImageCaptureSource) -> Option<BufferConstraints> {
        let target = self.target_of(source)?;
        Some(constraints(self.target_size(&target)))
    }

    fn new_session(&mut self, session: Session) {
        let size = self.target_of(&session.source()).map_or_else(|| (1, 1).into(), |t| self.target_size(&t));
        self.capture.sessions.push(CaptureSession {
            session,
            damage: OutputDamageTracker::new(size, 1.0, Transform::Normal),
            size,
            pending: None,
            fresh: true,
        });
    }

    fn frame(&mut self, session: &SessionRef, frame: Frame) {
        match self.capture.sessions.iter_mut().find(|s| s.session == *session) {
            // One frame at a time per session.
            Some(s) if s.pending.is_none() => s.pending = Some(frame),
            _ => frame.fail(CaptureFailureReason::Unknown),
        }
    }

    fn frame_aborted(&mut self, frame: FrameRef) {
        for s in &mut self.capture.sessions {
            if s.pending.as_deref() == Some(&frame) {
                s.pending = None;
            }
        }
    }

    fn session_destroyed(&mut self, session: SessionRef) {
        if let Some(i) = self.capture.sessions.iter().position(|s| s.session == session) {
            let s = self.capture.sessions.remove(i);
            if let Some(frame) = s.pending {
                frame.fail(CaptureFailureReason::Stopped);
            }
        }
        self.capture.copy.cleanup();
    }
}
