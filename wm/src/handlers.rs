//! Wayland protocol handlers: seats and selections, surfaces (a toplevel
//! joins the layout with its first buffer), and the xdg shell.

use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    desktop::{
        PopupKeyboardGrab, PopupKind, PopupPointerGrab, PopupUngrabStrategy, Window, find_popup_root_surface,
        get_popup_toplevel_coords,
    },
    input::{
        Seat, SeatHandler, SeatState,
        dnd::{DnDGrab, DndGrabHandler, GrabType, Source},
        pointer::{CursorImageStatus, Focus},
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            Client, Resource,
            protocol::{wl_buffer, wl_output::WlOutput, wl_seat, wl_surface::WlSurface},
        },
    },
    utils::{Rectangle, Serial},
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes, BufferAssignment, add_blocker, add_pre_commit_hook, get_parent, is_sync_subsurface, with_states},
        dmabuf::get_dmabuf,
        output::OutputHandler,
        pointer_constraints::PointerConstraintsHandler,
        selection::{
            SelectionHandler,
            data_device::{DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler, set_data_device_focus},
        },
        shell::xdg::{PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState},
        shm::{ShmHandler, ShmState},
    },
};

use crate::state::{ClientState, State, is_mapped};

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.seats.state
    }

    /// The app under a seat's pointer sets its cursor.
    fn cursor_image(&mut self, seat: &Seat<Self>, image: CursorImageStatus) {
        if let Some(data) = self.seats.list.iter_mut().find(|s| &s.seat == seat) {
            data.cursor = image;
        }
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
    }
}

impl PointerConstraintsHandler for State {}

/// Tablets aren't routed yet; the trait is what `cursor-shape` needs.
impl smithay::input::tablet::TabletSeatHandler for State {
    type ToolFocus = WlSurface;
}

impl SelectionHandler for State {
    type SelectionUserData = ();
}

impl DataDeviceHandler for State {
    fn data_device_state(&mut self) -> &mut DataDeviceState {
        &mut self.data_device_state
    }
}

impl DndGrabHandler for State {}

impl WaylandDndGrabHandler for State {
    fn dnd_requested<S: Source>(
        &mut self,
        source: S,
        _icon: Option<WlSurface>,
        seat: Seat<Self>,
        serial: Serial,
        type_: GrabType,
    ) {
        match type_ {
            GrabType::Pointer => {
                let pointer = seat.get_pointer().unwrap();
                let Some(start) = pointer.grab_start_data() else { return source.cancel() };
                let grab = DnDGrab::new_pointer(&self.display_handle, start, source, seat);
                pointer.set_grab(self, grab, serial, Focus::Keep);
            }
            GrabType::Touch => source.cancel(),
        }
    }
}

impl OutputHandler for State {}

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        // Implicit dma-buf fences must finish before the commit becomes visible.
        // Follow Smithay Anvil/niri: wait through calloop, never in the renderer.
        add_pre_commit_hook::<Self, _>(surface, |state, _, surface| {
            let dmabuf = with_states(surface, |data| {
                data.cached_state.get::<SurfaceAttributes>().pending().buffer.as_ref()
                    .and_then(|buffer| match buffer {
                        BufferAssignment::NewBuffer(buffer) => get_dmabuf(buffer).cloned().ok(),
                        _ => None,
                    })
            });
            if let Some(dmabuf) = dmabuf
                && let Ok((blocker, source)) = dmabuf.generate_blocker(smithay::reexports::calloop::Interest::READ)
                && let Some(client) = surface.client()
                && state.loop_handle.insert_source(source, move |_, _, state| {
                    let dh = state.display_handle.clone();
                    state.client_compositor_state(&client).blocker_cleared(state, &dh);
                    Ok(())
                }).is_ok()
            {
                add_blocker(surface, blocker);
            }
        });
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        if self.layer_commit(surface) {
            return;
        }
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(i) = self.unmapped.iter().position(|w| w.toplevel().is_some_and(|t| t.wl_surface() == &root)) {
                let window = self.unmapped[i].clone();
                window.on_commit();
                let toplevel = window.toplevel().unwrap();
                if is_mapped(&root) {
                    // Its first buffer: it joins the layout.
                    self.unmapped.remove(i);
                    self.add_window(window);
                } else if !toplevel.is_initial_configure_sent() {
                    // The first commit asks for a configure, with the size
                    // it will get in the layout; one floating over its
                    // parent (a dialog) picks its own.
                    if self.parent_of(&window).is_none() {
                        let ws = self.seats[self.home_seat(&window)].workspace;
                        let size = self.workspaces[&ws].new_column_size();
                        toplevel.with_pending_state(|s| s.size = Some(size));
                    }
                    toplevel.send_configure();
                }
            } else if let Some(window) = self.window_for_surface(&root) {
                window.on_commit();
                self.update_toplevel(&window);
                if !is_mapped(&root) {
                    // A null buffer unmaps it: out of the layout until it
                    // shows something again, after a new initial configure.
                    self.remove_window(&window);
                    self.unmapped.push(window);
                }
            }
        }

        self.popups.commit(surface);
        if let Some(PopupKind::Xdg(popup)) = self.popups.find_popup(surface)
            && !popup.is_initial_configure_sent()
        {
            popup.send_configure().expect("initial configure failed");
        }
    }
}

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    // It joins the layout with its first buffer (see `commit`).
    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        self.unmapped.push(Window::new_wayland_window(surface));
    }

    /// Full screen on the screen its column is on, whichever it names: a
    /// window belongs to its workspace. Asked before it shows, it is
    /// noted and done when it joins the layout (`add_window`).
    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<WlOutput>) {
        self.fullscreen_request_for(&surface, true);
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        self.fullscreen_request_for(&surface, false);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.unmapped.retain(|w| w.toplevel().is_some_and(|t| t != &surface));
        if let Some(window) = self.window_for_surface(surface.wl_surface()) {
            self.remove_window(&window);
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        self.unconstrain_layer_popup(&surface);
        surface.send_repositioned(token);
    }

    // Tiled windows don't move or resize themselves.
    fn move_request(&mut self, _surface: ToplevelSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}

    /// A menu takes the pointer and keyboard of the seat it was opened
    /// with (the client names it), and only that seat's: a click outside
    /// it closes it, the other seats carry on.
    fn grab(&mut self, surface: PopupSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let Some(seat) = Seat::<State>::from_resource(&seat) else { return };
        let kind = PopupKind::Xdg(surface);
        let Ok(root) = find_popup_root_surface(&kind) else { return };
        // Only for popups of what is laid out: a window or a layer surface.
        if self.window_for_surface(&root).is_none() && self.layer_of(&root).is_none() {
            return;
        }
        let Ok(mut grab) = self.popups.grab_popup(root, kind, &seat, serial) else { return };
        // Granted only for the click or key that opened it, or to a popup
        // nested in a grabbing one.
        let earlier = grab.previous_serial();
        if let Some(keyboard) = seat.get_keyboard() {
            if keyboard.is_grabbed() && !(keyboard.has_grab(serial) || keyboard.has_grab(earlier.unwrap_or(serial))) {
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            keyboard.set_focus(self, grab.current_grab(), serial);
            keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
        }
        if let Some(pointer) = seat.get_pointer() {
            if pointer.is_grabbed() && !(pointer.has_grab(serial) || pointer.has_grab(earlier.unwrap_or_else(|| grab.serial()))) {
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
        }
        if let Some(i) = self.seats.list.iter().position(|s| s.seat == seat) {
            self.seats.list[i].menu = Some(grab);
        }
    }
}

impl State {
    /// Keep a popup on screen, where its window is headed.
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else { return };
        let Some(window) = self.window_for_surface(&root) else { return };
        let Some(window_loc) = self.target_location(&window) else { return };
        let Some(ws) = self.workspace_of(&window) else { return };
        let mut target = Rectangle::from_size(self.workspaces[&ws].size());
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_loc;
        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}

smithay::delegate_dispatch2!(State);

impl State {
    fn fullscreen_request_for(&mut self, surface: &ToplevelSurface, on: bool) {
        match self.window_for_surface(surface.wl_surface()) {
            Some(window) => self.set_fullscreen(&window, Some(on), None),
            None => surface.with_pending_state(|s| {
                if on {
                    s.states.set(xdg_toplevel::State::Fullscreen);
                } else {
                    s.states.unset(xdg_toplevel::State::Fullscreen);
                }
            }),
        }
        // A configure answers it, changed or not.
        if surface.is_initial_configure_sent() {
            surface.send_configure();
        }
    }
}
