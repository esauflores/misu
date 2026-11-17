//! The compositor's state: seats, screens, workspaces, windows and focus.
//!
//! A seat is a user with their own pointer, keyboard focus, workspace, view
//! of it, and animation setting. Each screen shows one seat's workspace
//! (`Screen::viewer`); the other seats work on their own workspaces whether
//! or not anyone sees them. Animations are only for someone looking, so a
//! seat's changes animate when it has them on and a screen shows it (see
//! `State::animates`).
//!
//! Nested in another compositor there is only one set of host input devices,
//! so it drives one seat at a time (`Seats::active`); the others keep their
//! focus and pointer where they left them. On real hardware devices are
//! assigned to individual seats instead.
//!
//! A screen has a number (`ScreenId`, 1, 2, …) that everything refers to,
//! and an identity (make, model and serial from the monitor's EDID) used only
//! to recognise it when it comes back. Unplugged, a screen isn't removed: it
//! floats, out of sight, with its workspaces and windows as they were, and a
//! monitor plugged back in, into any port, gets it back.
//!
//! Workspaces have ids that never change, kept in one map; each screen holds
//! the order of its own as a list of ids. A screen shows the workspace of the
//! seat looking at it (`viewer`), which is always one of its own.

use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use smithay::{
    backend::renderer::utils::with_renderer_surface_state,
    desktop::{
        LayerSurface, PopupGrab, PopupManager, Space, Window, WindowSurfaceType, find_popup_root_surface, layer_map_for_output,
        space::SpaceElement,
    },
    input::{Seat, SeatState, keyboard::Keycode, pointer::CursorImageStatus},
    output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction, generic::Generic},
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            Display, DisplayHandle, Resource, backend::ClientData, protocol::wl_surface::WlSurface,
        },
    },
    utils::{Logical, Physical, Point, Rectangle, SERIAL_COUNTER, Serial, Size},
    wayland::{
        compositor::{CompositorClientState, CompositorState, with_states},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::{
            wlr_layer::{Layer, WlrLayerShellState},
            xdg::{XdgShellState, XdgToplevelSurfaceData},
        },
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use crate::{
    Config,
    ipc::Events,
    layout::{Anim, Direction, Workspace},
};

/// Set for the processes a seat starts (to the seat's index), and inherited
/// by theirs: their windows open on that seat's workspace.
pub const LAUNCH_ENV: &str = "WM_LAUNCH_SEAT";

/// The size workspaces take while no screen holds them, and of a screen
/// with no mode.
pub const FALLBACK_SIZE: (i32, i32) = (1280, 720);

pub struct State {
    pub config: Config,
    pub start_time: Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,
    pub loop_signal: LoopSignal,
    /// Quitting to be started again (`restart`, `RESTART_EXIT`).
    pub restart: bool,
    pub loop_handle: LoopHandle<'static, Self>,
    pub dmabuf: crate::backend::dmabuf::DmabufSupport,

    pub screens: HashMap<ScreenId, Screen>,
    /// The workspaces, the source of truth for where windows are.
    pub workspaces: HashMap<WorkspaceId, Workspace<Window>>,
    /// Toplevels with no buffer yet (or unmapped by a null one): not in any
    /// workspace until they show something.
    pub unmapped: Vec<Window>,
    next_workspace: u64,
    next_screen: u32,
    /// The plugged-in screens, left to right. Screens not here float.
    pub connected: Vec<ScreenId>,
    pub seats: Seats,
    /// Who `wm msg subscribe`s, and what they were last told.
    pub events: Events,
    /// Screenshots asked for, taken with the next frame (see `shot.rs`).
    pub shots: Vec<crate::backend::shot::Shot>,
    /// The processes seats spawned, by pid (see `launcher_of`).
    spawned: HashMap<u32, usize>,
    /// Capture by other programs (grim, screen sharing): see `capture.rs`.
    pub capture: crate::protocols::capture::Capture,
    /// Kept alive for its global: clients scale and crop their buffers
    /// (xwayland-satellite needs it).
    pub _viewporter: smithay::wayland::viewporter::ViewporterState,
    /// wp_fractional_scale: every surface's preferred scale is 1 (wm draws
    /// at 1×). Chromium needs it to draw a forced scale (Brave at 0.9) over
    /// its whole window, through wp_viewporter.
    pub _fractional_scale: smithay::wayland::fractional_scale::FractionalScaleManagerState,
    /// Touchpad pinches, holds and swipes, all for the app under the pointer.
    pub _pointer_gestures: smithay::wayland::pointer_gestures::PointerGesturesState,
    /// X11 apps' display (`:N`), while xwayland-satellite serves one.
    pub xwayland: Option<String>,
    /// Bumped when the input settings may have changed (`reload`): the
    /// backend sets its devices' options again.
    pub input_generation: u64,
    /// Bumped when the screen settings or the lid may have changed: the
    /// backend sets modes and turns screens on or off again.
    pub screen_generation: u64,
    /// The laptop's lid is closed: its panel is off.
    pub lid_closed: bool,
    pub session_lock: smithay::wayland::session_lock::SessionLockManagerState,
    /// The session lock, while locked (see `lock.rs`).
    pub locked: Option<crate::protocols::lock::Lock>,
    /// The cursor theme (see `cursor.rs`).
    pub cursors: crate::input::cursor::Cursors,
    /// Kept alive for its global: apps name their cursor's shape.
    pub _cursor_shape: smithay::wayland::cursor_shape::CursorShapeManagerState,
    /// Kept alive for its global: apps leave their title bars to wm.
    pub decorations: smithay::wayland::shell::xdg::decoration::XdgDecorationState,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    /// Kept alive for its wl_output and xdg-output globals.
    pub _output_manager_state: OutputManagerState,
    pub data_device_state: DataDeviceState,
    /// xdg-activation tokens (see `activation.rs`).
    pub activation: smithay::wayland::xdg_activation::XdgActivationState,
    /// xdg-foreign: a portal's dialog (a file chooser) set over the app's
    /// window that asked for it, which makes it a dialog here (see
    /// `parent_of`).
    pub foreign: smithay::wayland::xdg_foreign::XdgForeignState,
    /// Idle notify, per seat (see `handlers.rs`).
    pub idle: smithay::wayland::idle_notify::IdleNotifierState<Self>,
    /// Primary selection and data control (see `handlers.rs`).
    pub clipboard: crate::protocols::handlers::Clipboard,
    pub popups: PopupManager,
}

impl State {
    pub fn new(event_loop: &mut EventLoop<'static, Self>, display: Display<Self>, config: Config) -> Self {
        let dh = display.handle();
        let mut state = Self {
            start_time: Instant::now(),
            socket_name: Self::listen(display, event_loop),
            loop_signal: event_loop.get_signal(),
            restart: false,
            loop_handle: event_loop.handle(),
            dmabuf: crate::backend::dmabuf::DmabufSupport::default(),
            screens: HashMap::new(),
            workspaces: HashMap::new(),
            unmapped: Vec::new(),
            next_workspace: 1,
            next_screen: 1,
            connected: Vec::new(),
            seats: Seats::default(),
            events: Events::default(),
            shots: Vec::new(),
            spawned: HashMap::new(),
            capture: crate::protocols::capture::Capture::new(&dh),
            decorations: smithay::wayland::shell::xdg::decoration::XdgDecorationState::new::<State>(&dh),
            xwayland: None,
            cursors: crate::input::cursor::Cursors::default(),
            input_generation: 0,
            screen_generation: 0,
            lid_closed: false,
            session_lock: smithay::wayland::session_lock::SessionLockManagerState::new::<State, _>(&dh, |_| true),
            locked: None,
            _cursor_shape: smithay::wayland::cursor_shape::CursorShapeManagerState::new::<Self>(&dh),
            _viewporter: smithay::wayland::viewporter::ViewporterState::new::<Self>(&dh),
            _fractional_scale: smithay::wayland::fractional_scale::FractionalScaleManagerState::new::<Self>(&dh),
            _pointer_gestures: smithay::wayland::pointer_gestures::PointerGesturesState::new::<Self>(&dh),
            compositor_state: CompositorState::new::<Self>(&dh),
            xdg_shell_state: XdgShellState::new::<Self>(&dh),
            layer_shell_state: WlrLayerShellState::new::<Self>(&dh),
            shm_state: ShmState::new::<Self>(&dh, vec![]),
            _output_manager_state: OutputManagerState::new_with_xdg_output::<Self>(&dh),
            data_device_state: DataDeviceState::new::<Self>(&dh),
            clipboard: crate::protocols::handlers::Clipboard::new(&dh),
            activation: smithay::wayland::xdg_activation::XdgActivationState::new::<Self>(&dh),
            foreign: smithay::wayland::xdg_foreign::XdgForeignState::new::<Self>(&dh),
            idle: smithay::wayland::idle_notify::IdleNotifierState::new(&dh, event_loop.handle()),
            popups: PopupManager::default(),
            display_handle: dh.clone(),
            config,
        };
        // Each seat starts on a workspace of its own, which the first screen
        // to appear takes.
        for _ in 0..state.config.seats.max(1) {
            let ws = state.new_workspace(None);
            state.seats.add(&dh, ws, &state.config.input.keyboard);
        }
        state
    }

    fn listen(display: Display<Self>, event_loop: &mut EventLoop<Self>) -> OsString {
        let socket = ListeningSocketSource::new_auto().expect("wayland socket");
        let name = socket.socket_name().to_os_string();
        let handle = event_loop.handle();
        handle
            .insert_source(socket, |stream, _, state| {
                match state.display_handle.insert_client(stream, Arc::new(ClientState::default())) {
                    // Which seat launched it, before it sees any global: a
                    // client a seat launched sees only that seat's wl_seat.
                    Ok(client) => {
                        let pid = client.get_credentials(&state.display_handle).ok().map(|c| c.pid as u32);
                        let seat = pid.and_then(|pid| state.launcher_of_pid(pid));
                        let _ = client.get_data::<ClientState>().unwrap().seat.set(seat);
                    }
                    Err(err) => tracing::warn!("client rejected: {err}"),
                }
            })
            .expect("socket source");
        handle
            .insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, state| {
                // Safety: the display is not dropped while the loop runs.
                unsafe { display.get_mut().dispatch_clients(state).unwrap() };
                Ok(PostAction::Continue)
            })
            .expect("display source");
        name
    }

    // Screens and workspaces

    /// A new workspace, laid out for `screen` (or the fallback size).
    fn new_workspace(&mut self, screen: Option<ScreenId>) -> WorkspaceId {
        let id = WorkspaceId(self.next_workspace);
        self.next_workspace += 1;
        let (area, size) = match screen {
            Some(screen) => (self.screen_area(screen), self.screen_size(screen)),
            None => (Rectangle::from_size(FALLBACK_SIZE.into()), FALLBACK_SIZE.into()),
        };
        let mut ws = Workspace::new(size);
        ws.set_area(area, size, Instant::now(), false);
        self.workspaces.insert(id, ws);
        id
    }

    /// A monitor appeared. One seen before (same identity, on any port) gets
    /// its floating screen back. A new one gets the next number and the
    /// workspaces no screen holds; failing those, the workspace of a seat no
    /// screen shows, which comes over from its screen with the seat, so a
    /// second monitor goes to the second seat; or else a fresh one.
    pub fn add_screen(&mut self, output: Output) -> ScreenId {
        let identity = identity_of(&output, self.connected.iter().map(|id| &self.screens[id]));
        let floating = self.screens.values().filter(|s| !self.connected.contains(&s.id));
        if let Some(id) = returning(&identity, floating) {
            self.screens.get_mut(&id).unwrap().set_output(output);
            self.connect(id);
            return id;
        }
        let id = ScreenId(self.next_screen);
        self.next_screen += 1;
        let mut workspaces: Vec<WorkspaceId> =
            self.workspaces.keys().filter(|&&w| self.screen_of(w).is_none()).copied().collect();
        workspaces.sort_unstable();
        if workspaces.is_empty() {
            let shown: Vec<WorkspaceId> = self.shown().map(|(_, w, _)| w).collect();
            let unseen = self.seats.list.iter().map(|s| s.workspace).find(|w| {
                !shown.contains(w) && self.screen_of(*w).is_some_and(|s| self.connected.contains(&s))
            });
            let ws = match unseen {
                Some(ws) => {
                    self.detach_workspace(ws);
                    ws
                }
                None => self.new_workspace(None),
            };
            workspaces.push(ws);
        }
        self.screens.insert(id, Screen::new(id, identity, output, workspaces));
        self.connect(id);
        id
    }

    /// Where a screen goes among the connected ones: its entry in the
    /// config's `screens` (those without one go last, in plug order).
    fn screen_rank(&self, id: ScreenId) -> usize {
        let screen = &self.screens[&id];
        crate::screens::find(&self.config.screens, &screen.output.name(), &screen.identity).unwrap_or(usize::MAX)
    }

    /// Put the connected screens in the config's order.
    pub fn order_screens(&mut self) {
        let mut connected = std::mem::take(&mut self.connected);
        connected.sort_by_key(|&id| self.screen_rank(id));
        self.connected = connected;
    }

    /// Whether the monitor at output `name` with `identity` should be off:
    /// the config says so, or it is a laptop panel with the lid closed.
    pub fn screen_off(&self, name: &str, identity: &str) -> bool {
        let configured = crate::screens::find(&self.config.screens, name, identity).is_some_and(|i| self.config.screens[i].off);
        configured || (self.lid_closed && crate::screens::is_internal(name))
    }

    /// The lid closed or opened.
    pub fn set_lid(&mut self, closed: bool) {
        if self.lid_closed != closed {
            tracing::info!("lid {}", if closed { "closed" } else { "opened" });
            self.lid_closed = closed;
            self.screen_generation += 1;
        }
    }

    /// Put a floating screen back among the connected ones, in the
    /// config's order (else on the right).
    pub fn connect(&mut self, id: ScreenId) {
        if self.connected.contains(&id) || !self.screens.contains_key(&id) {
            return;
        }
        let rank = self.screen_rank(id);
        let at = self.connected.iter().position(|&s| self.screen_rank(s) > rank).unwrap_or(self.connected.len());
        self.connected.insert(at, id);
        self.resize_screen(id);
        self.settle_viewers();
        tracing::info!("screen {} ({}) connected", id.0, self.screens[&id].identity);
    }

    /// A monitor went away. Its screen floats, out of sight, keeping its
    /// workspaces and windows until it comes back. Whoever was looking at it
    /// moves to the first screen left; seats working there unseen stay.
    pub fn disconnect(&mut self, id: ScreenId) {
        if !self.connected.contains(&id) {
            return;
        }
        self.connected.retain(|&s| s != id);
        if let Some(viewer) = self.screens[&id].viewer
            && let Some(&other) = self.connected.first()
        {
            self.go_to(viewer, self.shows(other));
        }
        tracing::info!("screen {} ({}) disconnected", id.0, self.screens[&id].identity);
    }

    /// Keep every screen's viewer on it: a seat that left the screen it
    /// looked at stops looking at it, and a connected screen nobody looks at
    /// gets the first seat no screen shows that is on one of its workspaces.
    fn settle_viewers(&mut self) {
        for screen in self.screens.values_mut() {
            if screen.viewer.is_some_and(|v| !screen.workspaces.contains(&self.seats[v].workspace)) {
                screen.viewer = None;
            }
        }
        for id in self.connected.clone() {
            if self.screens[&id].viewer.is_none() {
                let seat = (0..self.seats.list.len()).find(|&s| {
                    !self.is_shown(s) && self.screens[&id].workspaces.contains(&self.seats[s].workspace)
                });
                self.screens.get_mut(&id).unwrap().viewer = seat;
            }
        }
    }

    /// Lay out a screen's workspaces in its area (it appeared or resized,
    /// or a bar took or gave back room).
    pub fn resize_screen(&mut self, id: ScreenId) {
        let now = Instant::now();
        let (area, size) = (self.screen_area(id), self.screen_size(id));
        for ws in self.screens[&id].workspaces.clone() {
            let animate = self.animates_workspace(ws);
            self.ws(ws).set_area(area, size, now, animate);
        }
        self.resize_lock_surfaces();
    }

    pub fn screen_of(&self, ws: WorkspaceId) -> Option<ScreenId> {
        self.screens.values().find(|s| s.workspaces.contains(&ws)).map(|s| s.id)
    }

    /// The screen `seat` is on, or the leftmost while its workspace has none.
    pub(crate) fn screen_for(&self, seat: usize) -> Option<ScreenId> {
        self.screen_of(self.seats[seat].workspace).or_else(|| self.connected.first().copied())
    }

    pub fn screen_size(&self, id: ScreenId) -> Size<i32, Logical> {
        self.screens.get(&id).map_or(FALLBACK_SIZE.into(), Screen::size)
    }

    /// What of a screen its workspaces get (see `Screen::area`).
    pub fn screen_area(&self, id: ScreenId) -> Rectangle<i32, Logical> {
        self.screens.get(&id).map_or(Rectangle::from_size(FALLBACK_SIZE.into()), Screen::area)
    }

    /// The `index`th workspace (from 0) of `seat`'s screen, made if missing.
    pub fn nth_workspace(&mut self, seat: usize, index: usize) -> Option<WorkspaceId> {
        let screen = self.screen_for(seat)?;
        while self.screens[&screen].workspaces.len() <= index {
            let ws = self.new_workspace(Some(screen));
            self.screens.get_mut(&screen).unwrap().workspaces.push(ws);
        }
        Some(self.screens[&screen].workspaces[index])
    }

    /// Take `ws` off its screen, leaving a fresh one there if it was the last.
    fn detach_workspace(&mut self, ws: WorkspaceId) {
        let Some(from) = self.screen_of(ws) else { return };
        let leftover = (self.screens[&from].workspaces.len() == 1).then(|| self.new_workspace(Some(from)));
        let screen = self.screens.get_mut(&from).unwrap();
        screen.workspaces.retain(|&w| w != ws);
        screen.workspaces.extend(leftover);
    }

    /// The workspace a screen shows (when connected): its viewer's, or its
    /// first while nobody looks at it.
    pub fn shows(&self, id: ScreenId) -> WorkspaceId {
        let screen = &self.screens[&id];
        screen.viewer.map_or(screen.workspaces[0], |v| self.seats[v].workspace)
    }

    /// What a connected screen shows, and the seat whose view it uses.
    pub fn shown_on(&self, id: ScreenId) -> Option<(WorkspaceId, Option<usize>)> {
        self.connected.contains(&id).then(|| (self.shows(id), self.screens[&id].viewer))
    }

    /// What each connected screen shows, and the seat whose view it uses.
    pub fn shown(&self) -> impl Iterator<Item = (ScreenId, WorkspaceId, Option<usize>)> + '_ {
        self.connected.iter().map(|&id| (id, self.shows(id), self.screens[&id].viewer))
    }

    /// Whether a screen shows `seat`: one it looks at.
    pub fn is_shown(&self, seat: usize) -> bool {
        self.shown().any(|(_, _, s)| s == Some(seat))
    }

    /// Screen `id` shows `ws` now, through `viewer`: the workspace it slides
    /// from and how far down `ws` still is, while it does. Going to a later
    /// workspace, the new one comes up from below; to an earlier one, down
    /// from above.
    pub fn slide(&mut self, id: ScreenId, ws: WorkspaceId, viewer: usize, now: Instant) -> Option<(WorkspaceId, f64)> {
        let animate = self.seats.list.get(viewer).is_some_and(|s| s.animate);
        let height = self.screen_size(id).h as f64;
        let screen = self.screens.get_mut(&id)?;
        if screen.shown != Some(ws) {
            let from = screen.shown.replace(ws);
            let index = |w: WorkspaceId| screen.workspaces.iter().position(|&x| x == w);
            screen.slide = from.filter(|_| animate).and_then(|from| {
                let down = index(ws)? > index(from)?;
                let mut anim = Anim::new(if down { height } else { -height });
                anim.set(0.0, now, true);
                Some((from, anim))
            });
        }
        let (from, anim) = screen.slide.as_ref()?;
        if anim.is_done(now) || !self.workspaces.contains_key(from) {
            screen.slide = None;
            return None;
        }
        Some((*from, anim.value(now)))
    }

    /// Whether `seat`'s changes animate: it has them on and a screen shows it.
    pub fn animates(&self, seat: usize) -> bool {
        self.seats[seat].animate && self.is_shown(seat)
    }

    /// Whether changes to `ws`'s columns animate: a screen shows it, through
    /// a seat with animations on.
    pub fn animates_workspace(&self, ws: WorkspaceId) -> bool {
        self.shown().any(|(_, w, s)| w == ws && s.is_some_and(|s| self.seats[s].animate))
    }

    fn ws(&mut self, id: WorkspaceId) -> &mut Workspace<Window> {
        self.workspaces.get_mut(&id).expect("workspace")
    }

    // Windows

    pub fn workspace_of(&self, window: &Window) -> Option<WorkspaceId> {
        self.workspaces.iter().find(|(_, w)| w.contains(window)).map(|(&id, _)| id)
    }

    /// Every window in a workspace.
    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.workspaces.values().flat_map(Workspace::windows)
    }

    /// The seats that have `window` focused.
    pub fn focusing<'a>(&'a self, window: &'a Window) -> impl Iterator<Item = usize> + 'a {
        self.seats.list.iter().enumerate().filter(move |(_, s)| s.focus.as_ref() == Some(window)).map(|(i, _)| i)
    }

    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<Window> {
        self.windows().find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface)).cloned()
    }

    /// The surface under `pos` in `seat`'s view of its own workspace, which
    /// may not be on screen.
    /// Layer surfaces of its screen are over and under the windows.
    pub fn surface_under(&self, seat: usize, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        if self.is_locked() {
            return self.lock_surface_under(seat, pos);
        }
        let layer = |layers: &[Layer]| self.layer_under(seat, layers, pos).map(|(_, s, p)| (s, p));
        layer(&[Layer::Overlay, Layer::Top])
            .or_else(|| {
                self.window_under(seat, pos).and_then(|(window, render_loc)| {
                    window
                        .surface_under(pos - render_loc.to_f64(), WindowSurfaceType::ALL)
                        .map(|(s, p)| (s, (p + render_loc).to_f64()))
                })
            })
            .or_else(|| layer(&[Layer::Bottom, Layer::Background]))
    }

    /// The window under `pos` for `seat` (its popups too), and where it is
    /// drawn.
    pub fn window_under(&self, seat: usize, pos: Point<f64, Logical>) -> Option<(Window, Point<i32, Logical>)> {
        if self.is_locked() {
            return None;
        }
        let ws = &self.workspaces[&self.seats[seat].workspace];
        let placed: Vec<_> = ws.positions(seat, Instant::now()).collect();
        // Past the area, only a full-screen window or a menu: a neighbour
        // column starts at its edge, where the bar may not take the pointer,
        // but a window's menus can reach over the bar.
        let in_area = ws.area().to_f64().contains(pos);
        placed.into_iter().rev().find_map(|(window, loc)| {
            // Windows are placed by their geometry; their surfaces start
            // before it (client-side shadows).
            let render_loc = loc - window.geometry().loc;
            let mut bbox = window.bbox_with_popups();
            bbox.loc += render_loc;
            let at = pos - render_loc.to_f64();
            let hit = if in_area || ws.is_fullscreen(window) {
                window.is_in_input_region(&at)
            } else {
                window.surface_under(at, WindowSurfaceType::POPUP).is_some()
            };
            (bbox.to_f64().contains(pos) && hit).then(|| (window.clone(), render_loc))
        })
    }

    /// The seat whose workspace a new window goes to: the one that launched
    /// its process (or one it descends from, which inherits `LAUNCH_ENV`,
    /// even through double forks), else the one the host drives.
    pub fn home_seat(&self, window: &Window) -> usize {
        window.toplevel().and_then(|t| self.launcher_of(t.wl_surface())).unwrap_or(self.seats.active)
    }

    /// The seat that launched `surface`'s client, recorded as it connected
    /// (`listen`, through `launcher_of_pid`).
    pub(crate) fn launcher_of(&self, surface: &WlSurface) -> Option<usize> {
        surface.client()?.get_data::<ClientState>()?.seat.get().copied().flatten()
    }

    /// The seat in the environment of process `pid` (see `LAUNCH_ENV`).
    ///
    /// The process, else the nearest of its ancestors, that a seat spawned
    /// or that has `LAUNCH_ENV`. Both are needed: Chromium-based apps (VS
    /// Code, Obsidian) overwrite their environment, and `sh -c` execs into
    /// the command, so wm's own record of the pid it spawned is what tells;
    /// a process that daemonized has left that tree, and its inherited
    /// environment tells instead.
    pub(crate) fn launcher_of_pid(&self, mut pid: u32) -> Option<usize> {
        let ours = std::process::id();
        let prefix = format!("{LAUNCH_ENV}=");
        for _ in 0..32 {
            if pid <= 1 || pid == ours {
                return None;
            }
            if let Some(&seat) = self.spawned.get(&pid) {
                return Some(seat).filter(|&s| s < self.seats.list.len());
            }
            if let Ok(environ) = std::fs::read(format!("/proc/{pid}/environ"))
                && let Some(value) = environ.split(|&b| b == 0).find_map(|var| var.strip_prefix(prefix.as_bytes()))
            {
                return std::str::from_utf8(value).ok()?.parse().ok().filter(|&seat| seat < self.seats.list.len());
            }
            pid = parent_pid(pid)?;
        }
        None
    }

    /// Remember that `seat` spawned `pid`, forgetting ones that have exited.
    pub(crate) fn note_spawn(&mut self, pid: u32, seat: usize) {
        self.spawned.retain(|pid, _| std::path::Path::new(&format!("/proc/{pid}")).exists());
        self.spawned.insert(pid, seat);
    }

    /// The window `window`'s app opened it for (a dialog's), if it is in
    /// the layout.
    pub fn parent_of(&self, window: &Window) -> Option<Window> {
        let parent = window.toplevel()?.parent()?;
        self.window_for_surface(&parent)
    }

    /// A toplevel showed its first buffer: it joins its home seat's
    /// workspace, focused there. One with a parent (a dialog) floats over it
    /// instead, focused by whoever focuses its parent.
    pub fn add_window(&mut self, window: Window) {
        let now = Instant::now();
        window.user_data().insert_if_missing(|| crate::protocols::activation::MappedAt(now));
        self.list_toplevel(&window);
        // A dialog floats: over its parent, else (polkit's prompt, a
        // browser extension's window) centred on its seat's screen; then
        // `size_dialog` sizes it.
        let parent = self.parent_of(&window).filter(|p| self.workspace_of(p).is_some());
        let seat = parent.as_ref().and_then(|p| self.focusing(p).next()).unwrap_or_else(|| self.home_seat(&window));
        let ws = parent.as_ref().and_then(|p| self.workspace_of(p)).unwrap_or(self.seats[seat].workspace);
        if parent.is_some() || lone_dialog(&window) {
            self.ws(ws).add_float(window.clone(), parent.as_ref());
            self.size_dialog(&window);
            if self.seats[seat].workspace == ws {
                self.focus(seat, Some(window), true);
            } else {
                self.update_activation();
            }
            return;
        }
        let after = self.seats[seat].focus.clone().filter(|w| self.workspaces[&ws].contains(w));
        let animate = self.animates_workspace(ws);
        self.ws(ws).add(window.clone(), after.as_ref(), now, animate);
        self.focus(seat, Some(window.clone()), true);
        // Asked for before it showed (see `fullscreen_request`).
        if window.toplevel().is_some_and(|t| t.with_pending_state(|s| s.states.contains(xdg_toplevel::State::Fullscreen))) {
            self.set_fullscreen(&window, Some(true), Some(seat));
        }
    }

    /// A dialog takes 3/4 of its screen each way, as far as the size range
    /// it allows goes (see `layout::dialog_size`).
    pub fn size_dialog(&self, window: &Window) {
        let Some(ws) = self.workspace_of(window) else { return };
        let Some(toplevel) = window.toplevel() else { return };
        let workspace = &self.workspaces[&ws];
        if !workspace.is_floating(window) {
            return;
        }
        let (min, max) = size_range(window);
        let want = crate::layout::dialog_size(workspace.size(), min, max);
        // Has it, or was asked already and took what it could.
        if window.geometry().size != want && toplevel.with_pending_state(|s| s.size) != Some(want) {
            toplevel.with_pending_state(|s| s.size = Some(want));
            toplevel.send_pending_configure();
        }
    }

    /// Make `window` full screen or not (`None`: the other way). `by`, the
    /// seat that asked, has its view brought to it; else every seat
    /// focusing it.
    pub fn set_fullscreen(&mut self, window: &Window, on: Option<bool>, by: Option<usize>) {
        let now = Instant::now();
        let Some(ws) = self.workspace_of(window) else { return };
        let animate = self.animates_workspace(ws);
        if !self.ws(ws).set_fullscreen(window, on, now, animate) {
            return;
        }
        let seats: Vec<usize> = match by {
            Some(seat) => vec![seat],
            None => self.focusing(window).collect(),
        };
        for seat in seats {
            if self.seats[seat].workspace == ws {
                self.reveal(seat, ws, window, now);
            }
        }
    }

    /// Whether `seat`'s view has a full-screen window covering its screen
    /// (the bar gives way).
    pub fn covered(&self, seat: usize) -> bool {
        self.seats.list.get(seat).is_some_and(|s| self.workspaces[&s.workspace].covered(seat, Instant::now()))
    }

    /// `window` closed or unmapped: out of the layout and off the list.
    pub fn remove_window(&mut self, window: &Window) {
        self.take_out(window);
        self.unlist_toplevel(window);
    }

    /// `window` out of its workspace; the seats on it move on to its
    /// neighbour.
    fn take_out(&mut self, window: &Window) {
        let now = Instant::now();
        let Some(ws) = self.workspace_of(window) else { return };
        let animate = self.animates_workspace(ws);
        let next = self.ws(ws).remove(window, now, animate);
        for screen in self.screens.values_mut() {
            screen.space.unmap_elem(window);
        }
        for seat in self.focusing(window).collect::<Vec<_>>() {
            self.focus(seat, next.clone(), false);
        }
    }

    // Focus

    /// Give `seat`'s keyboard to `window`. With `reveal`, the seat's view
    /// scrolls to show it: only for explicit moves, never for a click on a
    /// window that is already visible.
    pub fn focus(&mut self, seat: usize, window: Option<Window>, reveal: bool) {
        let now = Instant::now();
        if reveal && let Some(w) = &window && let Some(ws) = self.workspace_of(w) {
            self.reveal(seat, ws, w, now);
        }
        // A window that closed isn't one to go back to.
        let still_there = self.seats[seat].focus.as_ref().is_some_and(|w| self.workspace_of(w).is_some());
        let data = &mut self.seats.list[seat];
        if still_there && data.focus != window {
            data.previous = data.focus.take();
        }
        data.focus = window;
        self.update_keyboard(seat);
        self.update_activation();
    }

    /// Let go of the grabs of menus that closed. Smithay ends them only
    /// with the seat's next event; until then the keyboard would stay off
    /// the window.
    pub fn end_menus(&mut self) {
        for seat in 0..self.seats.list.len() {
            let Some(menu) = self.seats[seat].menu.clone() else { continue };
            if !menu.has_ended() {
                continue;
            }
            self.seats.list[seat].menu = None;
            let ours = |serial: Serial| serial == menu.serial() || Some(serial) == menu.previous_serial();
            let data = &self.seats[seat];
            let (pointer, keyboard) = (data.seat.get_pointer().unwrap(), data.seat.get_keyboard().unwrap());
            if pointer.with_grab(|serial, _| ours(serial)) == Some(true) {
                pointer.unset_grab(self, SERIAL_COUNTER.next_serial(), smithay::backend::input::InputTime::now());
            }
            if keyboard.with_grab(|serial, _| ours(serial)) == Some(true) {
                keyboard.unset_grab(self);
            }
            self.update_keyboard(seat);
        }
    }

    /// Give `seat`'s keyboard to its layer surface, else its window.
    pub fn update_keyboard(&mut self, seat: usize) {
        let data = &self.seats[seat];
        let surface = match &data.layer_focus {
            _ if self.is_locked() => self.seat_lock_surface(seat).cloned(),
            Some(layer) => Some(layer.wl_surface().clone()),
            None => data.focus.as_ref().and_then(|w| w.toplevel()).map(|t| t.wl_surface().clone()),
        };
        let keyboard = data.seat.get_keyboard().unwrap();
        if keyboard.current_focus() != surface {
            // A menu open elsewhere holds the keyboard: its focus moving on
            // (to another window, workspace or layer surface) closes it.
            if let Some(current) = keyboard.current_focus()
                && let Some(popup) = self.popups.find_popup(&current)
                && let Ok(root) = find_popup_root_surface(&popup)
                && surface.as_ref() != Some(&root)
            {
                let popups: Vec<_> = PopupManager::popups_for_surface(&root).map(|(p, _)| p).collect();
                for popup in popups {
                    let _ = PopupManager::dismiss_popup(&root, &popup);
                }
            }
            keyboard.set_focus(self, surface, SERIAL_COUNTER.next_serial());
        }
    }

    /// A window is shown active while any seat has it focused.
    fn update_activation(&self) {
        for window in self.windows() {
            let focused = self.focusing(window).next().is_some();
            if window.set_activated(focused) && let Some(t) = window.toplevel() && t.is_initial_configure_sent() {
                t.send_pending_configure();
            }
        }
    }

    /// Back to the window `seat` focused before this one, on whichever
    /// workspace; again, and it's back here.
    pub fn focus_previous(&mut self, seat: usize) {
        let Some(previous) = self.seats[seat].previous.clone().filter(|w| self.workspace_of(w).is_some()) else { return };
        let from = self.seats[seat].focus.clone();
        self.activate(seat, &previous);
        // Going there may have passed by that workspace's own focus.
        if from.is_some() {
            self.seats.list[seat].previous = from;
        }
    }

    pub fn focus_dir(&mut self, seat: usize, dir: Direction) {
        let ws = &self.workspaces[&self.seats[seat].workspace];
        let next = match self.seats[seat].focus.as_ref().filter(|w| ws.contains(w)) {
            Some(w) => ws.neighbour(w, dir),
            None => ws.first(),
        };
        if next.is_some() {
            self.focus(seat, next, true);
        }
    }

    /// Focus the first or last column of `seat`'s workspace.
    pub fn focus_end(&mut self, seat: usize, last: bool) {
        let ws = &self.workspaces[&self.seats[seat].workspace];
        if let Some(target) = if last { ws.last() } else { ws.first() } {
            self.focus(seat, Some(target), true);
        }
    }

    pub fn move_dir(&mut self, seat: usize, dir: Direction) {
        let now = Instant::now();
        let Some(window) = self.seats[seat].focus.clone() else { return };
        let Some(ws) = self.workspace_of(&window) else { return };
        let animate = self.animates_workspace(ws);
        self.ws(ws).move_column(&window, dir, now, animate);
        self.reveal(seat, ws, &window, now);
    }

    /// Scroll `seat`'s view of workspace `ws` to `window`.
    fn reveal(&mut self, seat: usize, ws: WorkspaceId, window: &Window, now: Instant) {
        let animate = self.animates(seat);
        self.ws(ws).reveal(seat, window, now, animate);
    }

    // Workspaces

    /// Send `seat` to the `index`th workspace (from 0) of its screen.
    pub fn switch_workspace(&mut self, seat: usize, index: usize) {
        if let Some(target) = self.nth_workspace(seat, index)
            && target != self.seats[seat].workspace
        {
            self.go_to(seat, target);
        }
    }

    /// Put `seat` on `target`. A screen it looks at shows it there; leaving
    /// for another screen, it looks at that one if nobody does.
    pub(crate) fn go_to(&mut self, seat: usize, target: WorkspaceId) {
        if let Some(window) = self.seats[seat].focus.clone()
            && let Some(from) = self.workspace_of(&window)
        {
            self.seats.list[seat].left.insert(from, window);
        }
        let back = self.seats.list[seat].left.remove(&target);
        self.seats.list[seat].workspace = target;
        self.settle_viewers();
        let ws = &self.workspaces[&target];
        // The window it had there, if it is still there; else the first.
        let focus = [self.seats[seat].focus.clone(), back].into_iter().flatten().find(|w| ws.contains(w)).or_else(|| ws.first());
        self.focus(seat, focus, false);
    }

    /// Bring workspace `ws`, from wherever it is (a floating screen, say),
    /// onto the end of `seat`'s screen, and put the seat there. A screen that
    /// showed it shows another, and seats on it come along.
    pub fn take_workspace(&mut self, seat: usize, ws: WorkspaceId) -> Result<(), String> {
        if !self.workspaces.contains_key(&ws) {
            return Err(format!("no workspace {}", ws.0));
        }
        let to = self.screen_for(seat).ok_or("no screen to take it to")?;
        if self.screen_of(ws) != Some(to) {
            self.detach_workspace(ws);
            self.screens.get_mut(&to).unwrap().workspaces.push(ws);
            let (area, size, animate) = (self.screen_area(to), self.screen_size(to), self.animates_workspace(ws));
            self.ws(ws).set_area(area, size, Instant::now(), animate);
        }
        self.go_to(seat, ws);
        Ok(())
    }

    /// Send `seat`'s focused window to the `index`th workspace of its
    /// screen; with `follow`, the seat goes too, still focused on it.
    pub fn move_to_workspace(&mut self, seat: usize, index: usize, follow: bool) {
        let Some(window) = self.seats[seat].focus.clone() else { return };
        let Some(from) = self.workspace_of(&window) else { return };
        let Some(target) = self.nth_workspace(seat, index) else { return };
        if from == target {
            return;
        }
        // Every seat on it moves on to its neighbour; the window waits, unfocused.
        self.take_out(&window);
        let animate = self.animates_workspace(target);
        self.ws(target).add(window.clone(), None, Instant::now(), animate);
        self.update_activation();
        if follow {
            self.go_to(seat, target);
            self.focus(seat, Some(window), true);
        }
    }

    /// Read the config file again: bindings, the modifier and devices
    /// (seats stay as they are). A broken file changes nothing.
    pub fn reload(&mut self) {
        match crate::Config::load(self.config.flags.clone()) {
            Ok(new) => {
                let seats = self.config.seats;
                self.config = crate::Config { seats, backend: self.config.backend, ..new };
                tracing::info!("config reloaded: {} bindings", self.config.bindings.len());
                // Keyboards now; libinput devices with the backend's next turn.
                let keyboard = self.config.input.keyboard.clone();
                for seat in 0..self.seats.list.len() {
                    let handle = self.seats[seat].seat.get_keyboard().unwrap();
                    if let Err(err) = handle.set_xkb_config(self, keyboard.xkb()) {
                        tracing::warn!("keyboard layout {:?} not set: {err:?}", keyboard.layout);
                    }
                    handle.change_repeat_info(keyboard.repeat_rate, keyboard.repeat_delay);
                }
                self.input_generation += 1;
                self.screen_generation += 1;
                self.order_screens();
            }
            Err(err) => tracing::warn!("config not reloaded: {err}"),
        }
    }

    /// `seat`'s animations on, off or toggled (None).
    pub fn set_animations(&mut self, seat: usize, on: Option<bool>) {
        let data = &mut self.seats[seat];
        data.animate = on.unwrap_or(!data.animate);
        let (animate, ws) = (data.animate, data.workspace);
        if !animate {
            let shown = self.is_shown(seat);
            let workspace = self.ws(ws);
            workspace.finish_view(seat);
            if shown {
                workspace.finish();
            }
        }
        tracing::info!("seat{seat}: animations {}", if animate { "on" } else { "off" });
    }

    /// Show the seat the host drives (nested: one set of devices, so you see
    /// the seat you're driving): it looks at the screen it is on.
    pub fn show_active_seat(&mut self) {
        let active = self.seats.active;
        if let Some(id) = self.screen_of(self.seats[active].workspace) {
            self.screens.get_mut(&id).unwrap().viewer = Some(active);
        }
        self.settle_viewers();
    }

    /// Put each screen's windows where its viewer sees them at `now`; a
    /// screen holds only its own, from its own (0, 0), so nothing spills
    /// onto the next one. Floating screens hold none.
    pub fn update_space(&mut self, now: Instant) {
        for id in self.screens.keys().copied().collect::<Vec<_>>() {
            let placed: Vec<(Window, Point<i32, Logical>)> = match self.shown_on(id) {
                Some((ws, viewer)) => self.workspaces[&ws]
                    .positions(viewer.unwrap_or(NOBODY), now)
                    .map(|(w, p)| (w.clone(), p))
                    .collect(),
                None => Vec::new(),
            };
            let space = &mut self.screens.get_mut(&id).unwrap().space;
            let gone: Vec<Window> =
                space.elements().filter(|w| !placed.iter().any(|(p, _)| p == *w)).cloned().collect();
            for window in gone {
                space.unmap_elem(&window);
            }
            for (window, loc) in placed {
                if space.element_location(&window) != Some(loc) {
                    // Floating windows (placed last) go on top.
                    let floating = self.workspaces.values().any(|ws| ws.is_floating(&window));
                    space.map_element(window, loc, floating);
                }
            }
        }
    }

    /// Where `window` is headed on its screen: as the seat looking at its
    /// workspace sees it, else a seat working there, else from the start.
    pub fn target_location(&self, window: &Window) -> Option<Point<i32, Logical>> {
        let id = self.workspace_of(window)?;
        let viewer = self.shown().find(|(_, w, _)| *w == id).and_then(|(_, _, v)| v);
        let seat = viewer.or_else(|| self.seats.list.iter().position(|s| s.workspace == id)).unwrap_or(NOBODY);
        let ws = &self.workspaces[&id];
        let (_, rect) = ws.targets().find(|(w, _)| *w == window)?;
        Some(rect.loc - Point::from((ws.view_target(seat).round() as i32, 0)))
    }

    /// Windows someone is working with or looking at: on a seat's workspace
    /// or a screen. They keep getting frame callbacks, on screen or not.
    pub fn live_windows(&self) -> impl Iterator<Item = &Window> {
        self.workspaces
            .iter()
            .filter(|(id, _)| {
                self.seats.list.iter().any(|s| s.workspace == **id) || self.shown().any(|(_, w, _)| w == **id)
            })
            .flat_map(|(_, ws)| ws.windows())
    }
}

/// Whether `surface` has a buffer: a toplevel shows once it has.
pub fn is_mapped(surface: &WlSurface) -> bool {
    with_renderer_surface_state(surface, |s| s.buffer().is_some()).unwrap_or(false)
}

/// A window's id for the control socket, unique for the compositor's life.
pub fn window_id(window: &Window) -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    window.user_data().get_or_insert(|| WindowId(NEXT.fetch_add(1, Ordering::Relaxed))).0
}

struct WindowId(u64);

pub fn window_info(window: &Window) -> (Option<String>, Option<String>) {
    let Some(toplevel) = window.toplevel() else { return (None, None) };
    with_states(toplevel.wl_surface(), |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .map(|d| {
                let d = d.lock().unwrap();
                (d.app_id.clone(), d.title.clone())
            })
            .unwrap_or_default()
    })
}

/// Per-client data.
#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    /// The seat that launched it (see `State::launcher_of`), set as it
    /// connects; `None` inside: no seat did (systemd, wm itself).
    pub seat: std::sync::OnceLock<Option<usize>>,
}

impl ClientData for ClientState {}

// Seats

/// A seat index no seat has, for a screen nobody looks at: its view of any
/// workspace is at the start.
pub const NOBODY: usize = usize::MAX;

pub struct SeatData {
    pub seat: Seat<State>,
    pub intercepted_keys: HashSet<Keycode>,
    /// The workspace it is on.
    pub workspace: WorkspaceId,
    pub focus: Option<Window>,
    /// The window it focused before `focus` (`focus previous` goes back).
    pub previous: Option<Window>,
    /// The window it had focused on each workspace it left, to come back to.
    pub left: HashMap<WorkspaceId, Window>,
    /// A layer surface with its keyboard instead of `focus` (fuzzel, say).
    pub layer_focus: Option<LayerSurface>,
    /// The menu (popup grab) holding its pointer and keyboard.
    pub menu: Option<PopupGrab<State>>,
    /// Its own setting; see `State::animates`.
    pub animate: bool,
    /// The cursor its pointer shows: what the app under it asks for.
    pub cursor: CursorImageStatus,
    /// Whether its pointer has moved or clicked yet: until it has, only the
    /// seat itself sees it, so an idle seat leaves no pointer on screen.
    pub pointer_used: bool,
    /// The surface its pointer was last sent over, with that surface's place.
    pub pointer_under: Option<(WlSurface, Point<f64, Logical>)>,
}

#[derive(Default)]
pub struct Seats {
    pub state: SeatState<State>,
    pub list: Vec<SeatData>,
    /// The seat the nested backend's input goes to.
    pub active: usize,
}

impl Seats {
    /// A new seat, on `workspace`.
    pub fn add(&mut self, dh: &DisplayHandle, workspace: WorkspaceId, keyboard: &crate::input::settings::Keyboard) -> usize {
        let index = self.list.len();
        let mut seat = self.state.new_wl_seat(dh, format!("seat{index}"));
        let added = seat.add_keyboard(keyboard.xkb(), keyboard.repeat_delay, keyboard.repeat_rate);
        if added.is_err() {
            tracing::warn!("keyboard layout {:?} not found: us instead", keyboard.layout);
            seat.add_keyboard(Default::default(), keyboard.repeat_delay, keyboard.repeat_rate).expect("keyboard");
        }
        seat.add_pointer();
        self.list.push(SeatData {
            seat,
            intercepted_keys: HashSet::new(),
            workspace,
            focus: None,
            previous: None,
            left: HashMap::new(),
            layer_focus: None,
            menu: None,
            animate: true,
            cursor: CursorImageStatus::default_named(),
            pointer_used: false,
            pointer_under: None,
        });
        index
    }
}

impl std::ops::Index<usize> for Seats {
    type Output = SeatData;
    fn index(&self, seat: usize) -> &SeatData {
        &self.list[seat]
    }
}

impl std::ops::IndexMut<usize> for Seats {
    fn index_mut(&mut self, seat: usize) -> &mut SeatData {
        &mut self.list[seat]
    }
}

// Screens

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScreenId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkspaceId(pub u64);

/// What tells a monitor apart: make, model and serial. Without a serial, or
/// when an identical monitor (`twin`) is connected already, the connector
/// too, since that is all that tells them apart.
pub fn identity(make: &str, model: &str, serial: &str, connector: &str, twin: bool) -> String {
    let serial = serial.trim();
    let base = [make.trim(), model.trim(), serial].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
    if serial.is_empty() || twin { format!("{base} @{connector}") } else { base }
}

/// A physical connector's stable device path, independent of DRM card numbers.
pub struct ConnectorId(pub String);

/// The size range a toplevel asks for: its min and max sizes, 0 for no limit.
fn size_range(window: &Window) -> (Size<i32, Logical>, Size<i32, Logical>) {
    let Some(toplevel) = window.toplevel() else { return Default::default() };
    with_states(toplevel.wl_surface(), |states| {
        let mut cached = states.cached_state.get::<smithay::wayland::shell::xdg::SurfaceCachedState>();
        let current = cached.current();
        (current.min_size, current.max_size)
    })
}

/// Whether a window with no parent is still a dialog: one fixed size
/// (polkit's prompt) or a browser extension's window (Bitwarden's popup).
fn lone_dialog(window: &Window) -> bool {
    let (min, max) = size_range(window);
    crate::layout::fixed_size(min, max) || window_info(window).0.is_some_and(|id| crate::layout::extension_window(&id))
}

/// `pid`'s parent, from `/proc/PID/stat` (after the parenthesised name,
/// which may hold spaces: state, then the parent).
fn parent_pid(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse().ok()
}

/// The identity `output` gets, given the screens connected now.
pub fn identity_of<'a>(output: &Output, connected: impl IntoIterator<Item = &'a Screen>) -> String {
    let p = output.physical_properties();
    let name = output.name();
    let connector = output.user_data().get::<ConnectorId>().map_or(name.as_str(), |id| id.0.as_str());
    let alone = identity(&p.make, &p.model, &p.serial_number, connector, false);
    let twin = connected.into_iter().any(|s| s.identity == alone);
    identity(&p.make, &p.model, &p.serial_number, connector, twin)
}

/// The floating screen with this identity, if any: the monitor is back.
pub fn returning<'a>(identity: &str, floating: impl IntoIterator<Item = &'a Screen>) -> Option<ScreenId> {
    floating.into_iter().find(|s| s.identity == identity).map(|s| s.id)
}

pub struct Screen {
    pub id: ScreenId,
    pub identity: String,
    /// The monitor; kept while floating, for its name and size.
    pub output: Output,
    /// Its workspaces, in order; never empty.
    pub workspaces: Vec<WorkspaceId>,
    /// The seat looking at it, always on one of its workspaces: the screen
    /// shows that workspace, with that seat's view. None while no seat is
    /// free to look (it shows its first workspace then).
    pub viewer: Option<usize>,
    /// What it shows, with its own output at (0, 0); see
    /// `State::update_space`.
    pub space: Space<Window>,
    /// The workspace it showed last frame, and the slide from it to the
    /// one it shows now: how far down that one still is.
    pub shown: Option<WorkspaceId>,
    pub slide: Option<(WorkspaceId, Anim)>,
}

impl Screen {
    pub fn new(id: ScreenId, identity: String, output: Output, workspaces: Vec<WorkspaceId>) -> Self {
        let mut space = Space::default();
        space.map_output(&output, (0, 0));
        Self { id, identity, output, workspaces, viewer: None, space, shown: None, slide: None }
    }

    /// The 1-based number of `ws` here.
    pub fn number_of(&self, ws: WorkspaceId) -> Option<usize> {
        self.workspaces.iter().position(|&w| w == ws).map(|i| i + 1)
    }

    /// The monitor came back as `output`: the screen shows on it from now on.
    pub fn set_output(&mut self, output: Output) {
        self.space.unmap_output(&self.output);
        self.space.map_output(&output, (0, 0));
        self.output = output;
    }

    /// The part its workspaces get: all of it but what layer surfaces (a
    /// bar) keep for themselves.
    pub fn area(&self) -> Rectangle<i32, Logical> {
        let mut map = layer_map_for_output(&self.output);
        map.arrange();
        let zone = map.non_exclusive_zone();
        if zone.size.w > 0 && zone.size.h > 0 { zone } else { Rectangle::from_size(self.size()) }
    }

    /// Its size in logical pixels, from the monitor's mode, scale and
    /// rotation.
    pub fn size(&self) -> Size<i32, Logical> {
        let Some(mode) = self.output.current_mode() else { return FALLBACK_SIZE.into() };
        let scale = self.output.current_scale().fractional_scale();
        self.output.current_transform().transform_size(mode.size).to_f64().to_logical(scale).to_i32_round()
    }
}

/// An output of wm's own making (the nested window, a virtual screen), with
/// one mode of `size`.
pub fn new_output(name: &str, model: &str, serial: &str, size: Size<i32, Physical>) -> Output {
    let output = Output::new(
        name.into(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "wm".into(),
            model: model.into(),
            serial_number: serial.into(),
        },
    );
    let mode = OutputMode { size, refresh: 60_000 };
    output.change_current_state(Some(mode), None, None, None);
    output.set_preferred(mode);
    output
}
