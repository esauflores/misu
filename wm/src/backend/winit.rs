//! The nested backend: the compositor runs in a window of the host's.

use std::time::{Duration, Instant};

use smithay::{
    backend::{
        renderer::{
            ImportDma,
            damage::OutputDamageTracker,
            gles::GlesRenderer,
        },
        winit::{self, WinitEvent},
    },
    output::Mode,
    reexports::calloop::EventLoop,
    utils::{Rectangle, Transform},
};

use crate::{
    backend::{BACKGROUND, render_node},
    state::{State, new_output},
};

pub fn init(event_loop: &mut EventLoop<State>, state: &mut State) -> Result<(), Box<dyn std::error::Error>> {
    let (mut backend, winit) = winit::init::<GlesRenderer>()?;
    // wm draws each seat's cursor; the host's would be one more.
    backend.window().set_cursor_visible(false);
    let node = render_node(backend.renderer());
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
        WinitEvent::Input(event) => state.process_seat_input_event(event, state.seats.active, screen),
        WinitEvent::Redraw => {
            let now = Instant::now();
            state.dmabuf.process_imports(|dmabuf| backend.renderer().import_dmabuf(dmabuf, None).is_ok());
            state.update_space(now);
            {
                let (renderer, mut framebuffer) = backend.bind().unwrap();
                let elements = state.screen_elements(renderer, screen, now);
                if let Err(err) = damage_tracker.render_output(renderer, &mut framebuffer, 0, &elements, BACKGROUND) {
                    tracing::warn!("render failed: {err:?}");
                }
            }
            let size = backend.window_size();
            if let Err(err) = backend.submit(Some(&[Rectangle::from_size(size)])) {
                tracing::warn!("submit failed: {err:?}");
            }
            state.take_shots(backend.renderer());

            state.send_frames(&output, Duration::ZERO);
            state.after_frame();
            // The host paces frames; keep asking for them.
            backend.window().request_redraw();
        }
        WinitEvent::CloseRequested => state.loop_signal.stop(),
        _ => {}
    })?;
    Ok(())
}
