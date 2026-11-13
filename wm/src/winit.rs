//! The nested backend: the compositor runs in a window of the host's.

use std::time::{Duration, Instant};

use smithay::{
    backend::{
        egl::EGLDevice,
        renderer::{
            ImportDma,
            damage::OutputDamageTracker,
            element::{Kind, solid::SolidColorRenderElement},
            gles::GlesRenderer,
        },
        winit::{self, WinitEvent},
    },
    output::Mode,
    reexports::calloop::EventLoop,
    utils::{Point, Rectangle, Transform},
};

use crate::state::{CURSOR_SIZE, MARK_HEIGHT, NOBODY, ScreenId, State, new_output};

pub(crate) const BACKGROUND: [f32; 4] = [0.08, 0.08, 0.1, 1.0];

pub fn init(event_loop: &mut EventLoop<State>, state: &mut State) -> Result<(), Box<dyn std::error::Error>> {
    let (mut backend, winit) = winit::init::<GlesRenderer>()?;
    let node = EGLDevice::device_for_display(backend.renderer().egl_context().display())
        .and_then(|device| device.try_get_render_node()).ok().flatten();
    state.dmabuf.enable(&state.display_handle, 0, node, backend.renderer().dmabuf_formats())?;
    let output = new_output("winit", "nested", "0", backend.window_size());
    let _global = output.create_global::<State>(&state.display_handle);
    output.change_current_state(None, Some(Transform::Flipped180), None, Some((0, 0).into()));
    let screen = state.add_screen(output.clone());

    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    event_loop.handle().insert_source(winit, move |event, _, state| match event {
        WinitEvent::Resized { size, .. } => {
            output.change_current_state(Some(Mode { size, refresh: 60_000 }), None, None, None);
            state.resize_screen(screen);
        }
        WinitEvent::Input(event) => state.process_input_event(event, screen),
        WinitEvent::Redraw => {
            let now = Instant::now();
            state.dmabuf.process_imports(|dmabuf| backend.renderer().import_dmabuf(dmabuf, None).is_ok());
            state.update_space(now);
            // Unplugged (`wm msg screen-remove`), it shows only the background.
            let overlay = overlay(state, screen, now);
            let spaces = if state.connected.contains(&screen) { vec![&state.screens[&screen].space] } else { Vec::new() };
            {
                let (renderer, mut framebuffer) = backend.bind().unwrap();
                let result = smithay::desktop::space::render_output::<_, SolidColorRenderElement, _, _>(
                    &output,
                    renderer,
                    &mut framebuffer,
                    1.0,
                    0,
                    spaces,
                    &overlay,
                    &mut damage_tracker,
                    BACKGROUND,
                );
                if let Err(err) = result {
                    tracing::warn!("render failed: {err:?}");
                }
            }
            let size = backend.window_size();
            if let Err(err) = backend.submit(Some(&[Rectangle::from_size(size)])) {
                tracing::warn!("submit failed: {err:?}");
            }

            let elapsed = state.start_time.elapsed();
            // Off-screen seats' windows keep drawing too.
            for window in state.live_windows() {
                window.send_frame(&output, elapsed, Some(Duration::ZERO), |_, _| Some(output.clone()));
            }
            for screen in state.screens.values_mut() {
                screen.space.refresh();
            }
            state.popups.cleanup();
            let _ = state.display_handle.flush_clients();
            // The host paces frames; keep asking for them.
            backend.window().request_redraw();
        }
        WinitEvent::CloseRequested => state.loop_signal.stop(),
        _ => {}
    })?;
    Ok(())
}

/// The pointers of the seats on the screen's workspace, and a mark in each
/// seat's colour above the window it focuses there, in screen coordinates.
pub(crate) fn overlay(state: &mut State, screen: ScreenId, now: Instant) -> Vec<SolidColorRenderElement> {
    let mut elements = Vec::new();
    let Some((shown, viewer)) = state.shown_on(screen) else { return elements };
    let viewer = viewer.unwrap_or(NOBODY);
    let ws = &state.workspaces[&shown];
    let viewer_view = ws.view(viewer, now);
    let placed: Vec<_> = ws.positions(viewer, now).map(|(w, p)| (w.clone(), p)).collect();
    for (index, data) in state.seats.list.iter_mut().enumerate() {
        if data.workspace != shown {
            continue;
        }
        // A seat's pointer is on its own view of the workspace; draw it over
        // what it points at in the viewer's.
        let pointer = data.seat.get_pointer().unwrap().current_location();
        let half = (CURSOR_SIZE / 2) as f64;
        let x = pointer.x + ws.view(index, now) - viewer_view;
        let at = Point::from((x - half, pointer.y - half)).to_i32_round();
        elements.push(SolidColorRenderElement::from_buffer(&data.cursor, at.to_physical(1), 1.0, 1.0, Kind::Cursor));

        if let Some((window, loc)) = placed.iter().find(|(w, _)| Some(w) == data.focus.as_ref()) {
            let width = window.geometry().size.w.max(1);
            data.mark.resize((width, MARK_HEIGHT));
            // Two rows fit in the gap above the window, two in the one below.
            let row = (MARK_HEIGHT + 2) * (index as i32 % 2);
            let y = if index % 4 < 2 {
                loc.y - (MARK_HEIGHT + 2) - row
            } else {
                loc.y + window.geometry().size.h + 2 + row
            };
            let at = Point::from((loc.x, y));
            elements.push(SolidColorRenderElement::from_buffer(&data.mark, at.to_physical(1), 1.0, 1.0, Kind::Unspecified));
        }
    }
    elements
}
