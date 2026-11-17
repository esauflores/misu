//! Direct hardware backend. The libseat session owns access to DRM and input;
//! each connector has its own damage-tracked compositor and pending page flip.

use crate::{
    backend::{BACKGROUND, render_node},
    state::{ConnectorId, ScreenId, State},
};
use smithay::{
    backend::{
        allocator::{
            Fourcc,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{
            DrmDevice, DrmDeviceFd, DrmEvent, DrmNode, NodeType,
            compositor::{DrmCompositor, FrameFlags, PrimaryPlaneElement},
            exporter::gbm::GbmFramebufferExporter,
        },
        egl::{EGLContext, EGLDisplay},
        input::{
            ButtonState, Event, InputEvent, InputTime, KeyState, KeyboardKeyEvent, PointerButtonEvent, Switch, SwitchState,
            SwitchToggleEvent,
        },
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            ImportDma,
            gles::GlesRenderer,
        },
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
        udev::{UdevBackend, UdevEvent},
    },
    input::keyboard::KeyboardSource,
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{
            EventLoop, LoopHandle, RegistrationToken, ping,
            timer::{TimeoutAction, Timer},
        },
        drm::control::{Device as _, ModeTypeFlags, connector, crtc},
        input::Libinput,
        rustix::fs::OFlags,
        udev,
        wayland_server::backend::GlobalId,
    },
    utils::DeviceFd,
};
use smithay_drm_extras::{display_info, drm_scanner::DrmScanner};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    os::fd::OwnedFd,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

type Error = Box<dyn std::error::Error>;
type Compositor = DrmCompositor<GbmAllocator<DeviceFd>, GbmFramebufferExporter<DeviceFd>, (), DeviceFd>;
struct Surface {
    screen: ScreenId,
    connector: connector::Handle,
    global: GlobalId,
    compositor: Compositor,
    pending: bool,
    next_frame: Instant,
    /// The first commit the queued frame shows, and the DRM sequence of the
    /// last frame shown (`FrameStats`).
    committed_at: Option<Instant>,
    sequence: Option<u32>,
    /// When this screen's last page flip sent its windows frame callbacks.
    flipped_at: Option<Instant>,
    /// When the queued frame was queued, and when to draw the next one
    /// (`DrawClock`): unset, at once.
    queued_at: Option<Instant>,
    draw_at: Option<Instant>,
    clock: super::DrawClock,
}
struct Gpu {
    name: String,
    identity: String,
    // Drop surfaces and renderer before the device and its session fd.
    surfaces: HashMap<crtc::Handle, Surface>,
    renderer: GlesRenderer,
    gbm: GbmDevice<DeviceFd>,
    scanner: DrmScanner,
    needs_scan: bool,
    drm: DrmDevice,
    token: RegistrationToken,
    _session_device: SessionDevice,
}

// Keep the original libseat descriptor until every DRM/GBM reference to its
// duplicate is gone. Even initialization failures must close through libseat.
struct SessionDevice {
    fd: Option<OwnedFd>,
    session: LibSeatSession,
}
impl Drop for SessionDevice {
    fn drop(&mut self) {
        if let Some(fd) = self.fd.take()
            && let Err(err) = self.session.close(fd)
        {
            tracing::warn!(?err, "session device close failed");
        }
    }
}
struct PhysicalInput {
    /// Kept to set its options again on `reload`.
    device: smithay::reexports::input::Device,
    seat: usize,
    source: KeyboardSource,
    buttons: HashSet<u32>,
    keys: HashSet<smithay::input::keyboard::Keycode>,
}
struct Hardware {
    gpus: HashMap<u64, Gpu>,
    session: LibSeatSession,
    handle: LoopHandle<'static, State>,
    input: Libinput,
    active: bool,
    devices: HashMap<u64, PathBuf>,
    callbacks_at: Instant,
    callback_output: Output,
    input_devices: HashMap<String, PhysicalInput>,
    /// The `State::input_generation` its devices' options are from.
    input_generation: u64,
    /// The `State::screen_generation` its screens are set up for.
    screen_generation: u64,
}

/// Keeps event sources alive and closes session devices before libseat exits.
pub struct Backend {
    hardware: Rc<RefCell<Hardware>>,
    tokens: Vec<RegistrationToken>,
}
impl Drop for Backend {
    fn drop(&mut self) {
        let mut hw = self.hardware.borrow_mut();
        hw.input.suspend();
        let handle = hw.handle.clone();
        for (_, mut gpu) in hw.gpus.drain() {
            handle.remove(gpu.token);
            // Paused, as on a VT switch, the screens keep their last frame
            // until the next session's (or the console's) replaces it:
            // switched off, a laptop panel makes both wait out its power
            // cycle, a second for a restart.
            gpu.drm.pause();
            drop(gpu);
        }
        drop(hw);
        for token in self.tokens.drain(..) {
            handle.remove(token);
        }
    }
}

pub fn init(event_loop: &mut EventLoop<'static, State>, state: &mut State) -> Result<Backend, Error> {
    let (session, notifier) = LibSeatSession::new()?;
    let udev = UdevBackend::new(session.seat())?;
    let mut input = Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
    input
        .udev_assign_seat(&session.seat())
        .map_err(|_| "could not assign libinput session seat")?;
    let active = session.is_active();
    if !active {
        input.suspend();
    }
    let hardware = Rc::new(RefCell::new(Hardware {
        gpus: HashMap::new(),
        session,
        handle: event_loop.handle(),
        input: input.clone(),
        active,
        devices: udev.device_list().map(|(id, path)| (id, path.to_owned())).collect(),
        callbacks_at: Instant::now(),
        input_devices: HashMap::new(),
        input_generation: 0,
        screen_generation: 0,
        callback_output: crate::state::new_output("unseen", "callbacks", "", (1280, 720).into()),
    }));
    let mut backend = Backend {
        hardware: hardware.clone(),
        tokens: Vec::new(),
    };
    let hw = hardware.clone();
    let shared = hardware.clone();
    backend
        .tokens
        .push(event_loop.handle().insert_source(notifier, move |event, _, state| {
            let mut hw = hw.borrow_mut();
            match event {
                SessionEvent::PauseSession => {
                    tracing::info!("session paused");
                    hw.active = false;
                    hw.input.suspend();
                    let devices = std::mem::take(&mut hw.input_devices);
                    for device in devices.into_values() {
                        device.release(state);
                    }
                    for gpu in hw.gpus.values_mut() {
                        gpu.drm.pause();
                    }
                }
                SessionEvent::ActivateSession => {
                    tracing::info!("session resumed");
                    if let Err(err) = hw.input.resume() {
                        tracing::warn!(?err, "libinput resume failed");
                    }
                    hw.active = true;
                    for gpu in hw.gpus.values_mut() {
                        if let Err(err) = gpu.drm.activate(true) {
                            tracing::warn!(?err, "DRM resume failed");
                            continue;
                        }
                        for surface in gpu.surfaces.values_mut() {
                            surface.pending = false;
                            // activate(true) disabled scanout: discard any page flip
                            // whose completion was lost while the VT was inactive.
                            if let Err(err) = surface.compositor.clear() {
                                tracing::warn!(?err, "output clear failed");
                            }
                            if let Err(err) = surface.compositor.reset_state() {
                                tracing::warn!(?err, "output reset failed");
                            }
                            surface.compositor.reset_buffers();
                            surface.next_frame = Instant::now();
                            surface.draw_at = None;
                            surface.queued_at = None;
                        }
                    }
                    let devices = hw.devices.clone();
                    for (id, path) in devices {
                        if let Err(err) = hw.add_gpu(id, &path, state, shared.clone()) {
                            tracing::warn!(?err, "GPU resume failed");
                        }
                    }
                    hw.rescan(state);
                }
            }
        })?);
    let hw = hardware.clone();
    backend.tokens.push(event_loop.handle().insert_source(
        LibinputInputBackend::new(input),
        move |event, _, state| {
            hw.borrow_mut().process_input(event, state);
            if let Some(redraw) = &state.redraw {
                redraw.ping();
            }
        },
    )?);
    // Commits and input draw at once (coalesced per loop iteration), so they
    // make the next flip rather than waiting for the poll.
    let (redraw, source) = ping::make_ping()?;
    state.redraw = Some(redraw);
    let hw = hardware.clone();
    backend.tokens.push(event_loop.handle().insert_source(source, move |_, _, state| {
        let mut hw = hw.borrow_mut();
        if hw.active {
            hw.render(state);
        }
    })?);
    // Initial enumeration and subsequent hotplug use exactly the same path.
    for (id, path) in udev.device_list() {
        if let Err(err) = hardware.borrow_mut().add_gpu(id, path, state, hardware.clone()) {
            tracing::warn!(?err, ?path, "GPU unavailable");
        }
    }
    let hw = hardware.clone();
    backend
        .tokens
        .push(event_loop.handle().insert_source(udev, move |event, _, state| {
            let mut backend = hw.borrow_mut();
            match event {
                UdevEvent::Added { device_id, path } => {
                    backend.devices.insert(device_id, path.clone());
                    if let Err(err) = backend.add_gpu(device_id, &path, state, hw.clone()) {
                        tracing::warn!(?err, "GPU hotplug failed");
                    }
                }
                UdevEvent::Changed { .. } => backend.rescan(state),
                UdevEvent::Removed { device_id } => {
                    backend.devices.remove(&device_id);
                    backend.remove_gpu(device_id, state);
                }
            }
        })?);
    // Poll damage at each output's refresh rate, never queueing unchanged
    // content or drawing over an outstanding page flip. The short timer also
    // paces callbacks for clients on workspaces no output currently shows.
    backend.tokens.push(
        event_loop
            .handle()
            .insert_source(Timer::immediate(), move |_, _, state| {
                let mut hw = hardware.borrow_mut();
                if !hw.active {
                    state.fail_shots("the session is paused (another VT)");
                    return TimeoutAction::ToDuration(Duration::from_millis(16));
                }
                if hw.input_generation != state.input_generation {
                    hw.input_generation = state.input_generation;
                    for input in hw.input_devices.values_mut() {
                        state.config.input.apply(&mut input.device);
                    }
                }
                if hw.screen_generation != state.screen_generation {
                    hw.screen_generation = state.screen_generation;
                    hw.rescan(state);
                }
                hw.render(state);
                let next = hw
                    .gpus
                    .values()
                    .flat_map(|gpu| gpu.surfaces.values())
                    .filter(|surface| !surface.pending)
                    .map(|surface| surface.next_frame)
                    .fold(hw.callbacks_at, Instant::min);
                TimeoutAction::ToInstant(next.max(Instant::now() + Duration::from_millis(1)))
            })?,
    );
    Ok(backend)
}

impl PhysicalInput {
    fn release(self, state: &mut State) {
        let keyboard = state.seats[self.seat].seat.get_keyboard().unwrap();
        // Use the normal filter so intercepted shortcuts also keep their
        // releases private when a keyboard is unplugged or the VT changes.
        for key in self.keys {
            state.keyboard_input(self.seat, self.source, key, KeyState::Released, InputTime::now());
        }
        let pressed = keyboard.pressed_keys();
        state.seats[self.seat]
            .intercepted_keys
            .retain(|key| pressed.contains(key));
        for button in self.buttons {
            state.pointer_button(self.seat, button, ButtonState::Released, InputTime::now());
        }
    }
}

impl Hardware {
    fn process_input(&mut self, event: InputEvent<LibinputInputBackend>, state: &mut State) {
        if let InputEvent::SwitchToggle { event } = &event {
            if SwitchToggleEvent::switch(event) == Some(Switch::Lid) {
                state.set_lid(SwitchToggleEvent::state(event) == SwitchState::On);
            }
            return;
        }
        let mut device = match &event {
            InputEvent::DeviceAdded { device } | InputEvent::DeviceRemoved { device } => device.clone(),
            InputEvent::Keyboard { event } => event.device(),
            InputEvent::PointerMotion { event } => event.device(),
            InputEvent::PointerMotionAbsolute { event } => event.device(),
            InputEvent::PointerButton { event } => event.device(),
            InputEvent::PointerAxis { event } => event.device(),
            InputEvent::GestureSwipeBegin { event } => event.device(),
            InputEvent::GestureSwipeUpdate { event } => event.device(),
            InputEvent::GestureSwipeEnd { event } => event.device(),
            InputEvent::GesturePinchBegin { event } => event.device(),
            InputEvent::GesturePinchUpdate { event } => event.device(),
            InputEvent::GesturePinchEnd { event } => event.device(),
            InputEvent::GestureHoldBegin { event } => event.device(),
            InputEvent::GestureHoldEnd { event } => event.device(),
            _ => return,
        };
        let id = device.sysname().to_owned();
        if matches!(event, InputEvent::DeviceRemoved { .. }) {
            if let Some(mut removed) = self.input_devices.remove(&id) {
                tracing::info!(device = %device.name(), seat = removed.seat, "input disconnected");
                // Another mouse on this seat may still hold the same button.
                removed.buttons.retain(|button| {
                    !self
                        .input_devices
                        .values()
                        .any(|d| d.seat == removed.seat && d.buttons.contains(button))
                });
                removed.release(state);
            }
            return;
        }
        if !self.active {
            return;
        }
        let seat = state.config.device_seat(&device.name());
        let data = self.input_devices.entry(id.clone()).or_insert_with(|| PhysicalInput {
            device: device.clone(),
            seat,
            source: KeyboardSource::new_auxiliary(),
            buttons: HashSet::new(),
            keys: HashSet::new(),
        });
        match event {
            InputEvent::DeviceAdded { .. } => {
                tracing::info!(device = %device.name(), seat, "input connected");
                state.config.input.apply(&mut device);
            }
            InputEvent::Keyboard { event } => {
                if event.state() == KeyState::Pressed {
                    data.keys.insert(event.key_code());
                } else {
                    data.keys.remove(&event.key_code());
                }
                if let Some(vt) =
                    state.keyboard_input(seat, data.source, event.key_code(), event.state(), Event::time(&event))
                    && let Err(err) = self.session.change_vt(vt)
                {
                    tracing::warn!(?err, vt, "VT switch failed");
                }
                device.led_update(state.seats[seat].seat.get_keyboard().unwrap().led_state().into());
            }
            InputEvent::PointerButton { event } => {
                let button = event.button_code();
                let pressed = event.state() == ButtonState::Pressed;
                let changed = if pressed {
                    data.buttons.insert(button)
                } else {
                    data.buttons.remove(&button)
                };
                if changed
                    && !self
                        .input_devices
                        .iter()
                        .any(|(other, d)| other != &id && d.seat == seat && d.buttons.contains(&button))
                {
                    state.pointer_button(seat, button, event.state(), Event::time(&event));
                }
            }
            event => {
                let screen = state.screen_for(seat).unwrap_or(ScreenId(0));
                state.process_seat_input_event(event, seat, screen);
            }
        }
    }

    fn add_gpu(&mut self, id: u64, path: &Path, state: &mut State, shared: Rc<RefCell<Self>>) -> Result<(), Error> {
        if DrmNode::from_dev_id(id)?.ty() != NodeType::Primary || self.gpus.contains_key(&id) || !self.active {
            return Ok(());
        }
        let device = udev::Device::from_devnum(udev::DeviceType::Character, id)?;
        let parent = device.parent().ok_or("GPU has no physical device path")?;
        let identity = device
            .property_value("ID_PATH")
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| parent.syspath().to_string_lossy().into_owned());
        let fd = self
            .session
            .open(path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)?;
        let device = SessionDevice {
            fd: Some(fd),
            session: self.session.clone(),
        };
        let fd = DrmDeviceFd::new(DeviceFd::from(device.fd.as_ref().unwrap().try_clone()?));
        let started = Instant::now();
        // Keep lit screens on: turning a laptop panel off makes it wait out
        // its power cycle before it comes back.
        let (drm, notifier) = DrmDevice::new(fd.clone(), false)?;
        let opened = started.elapsed();
        let gbm = GbmDevice::new(fd.device_fd())?;
        // SAFETY: GBM and the EGL context outlive the renderer and its surfaces.
        let display = unsafe { EGLDisplay::new(gbm.clone())? };
        let context = EGLContext::new(&display)?;
        let egl = started.elapsed();
        let renderer = unsafe { GlesRenderer::new(context)? };
        tracing::info!(?opened, ?egl, ready = ?started.elapsed(), "GPU {} started", path.display());
        let token = self.handle.insert_source(notifier, move |event, metadata, state| {
            let mut hw = shared.borrow_mut();
            let handle = hw.handle.clone();
            if let Some(gpu) = hw.gpus.get_mut(&id) {
                match event {
                    DrmEvent::VBlank(crtc) => {
                        if let Some(surface) = gpu.surfaces.get_mut(&crtc) {
                            if let Err(err) = surface.compositor.frame_submitted() {
                                tracing::warn!(?err, "page flip completion failed");
                            }
                            surface.pending = false;
                            let sequence = metadata.map(|m| m.sequence);
                            let gap = sequence.zip(surface.sequence).map(|(now, last)| now.wrapping_sub(last));
                            surface.sequence = sequence.or(surface.sequence);
                            state.frame_stats.shown(gap, surface.committed_at.take());
                            // Apps draw their next frame now, a whole refresh
                            // ahead of the next flip, instead of at a timer
                            // drifting against the screen.
                            let screen = surface.screen;
                            let now = Instant::now();
                            let output = state.screens.get(&screen).map(|screen| screen.output.clone());
                            let period = output
                                .as_ref()
                                .and_then(|output| output.current_mode())
                                .map_or(Duration::from_millis(16), |mode| Duration::from_secs_f64(1000.0 / mode.refresh.max(1) as f64));
                            // A frame queued before the refresh after the last
                            // flip was meant for it: a gap of more is a miss.
                            let aimed = surface.queued_at.take().zip(surface.flipped_at).is_some_and(|(queued, flipped)| queued < flipped + period);
                            if aimed && let Some(gap) = gap {
                                surface.clock.shown(gap > 1);
                            }
                            surface.flipped_at = Some(now);
                            if let Some(output) = output {
                                state.send_frames(&output, |shown| shown == Some(screen));
                            }
                            // The next frame waits for the updates this
                            // refresh brings, then makes the next flip.
                            let at = now + period - surface.clock.budget(period);
                            surface.draw_at = Some(at);
                            let shared = shared.clone();
                            let timer = handle.insert_source(Timer::from_deadline(at), move |_, _, state| {
                                let mut hw = shared.borrow_mut();
                                if hw.active {
                                    hw.render(state);
                                }
                                TimeoutAction::Drop
                            });
                            if let Err(err) = timer {
                                tracing::warn!(?err, "draw timer failed");
                                surface.draw_at = None;
                            }
                        }
                    }
                    DrmEvent::Error(err) => tracing::warn!(?err, "DRM event error"),
                }
                if gpu.needs_scan
                    && let Err(err) = gpu.scan(state, &handle)
                {
                    tracing::warn!(?err, "deferred connector scan failed");
                }
            }
            if hw.active {
                hw.render(state);
            }
        })?;
        let mut gpu = Gpu {
            name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            identity,
            surfaces: HashMap::new(),
            renderer,
            gbm,
            scanner: DrmScanner::new(),
            needs_scan: false,
            drm,
            token,
            _session_device: device,
        };
        if let Err(err) = gpu.scan(state, &self.handle) {
            tracing::warn!(?err, "initial connector scan failed");
        }
        self.gpus.insert(id, gpu);
        if state.dmabuf.device.is_none() {
            let renderer = &self.gpus[&id].renderer;
            let node = render_node(renderer).or_else(|| DrmNode::from_dev_id(id).ok());
            if let Err(err) = state.dmabuf.enable(&state.display_handle, id, node, renderer.dmabuf_formats()) {
                tracing::warn!(?err, "could not advertise dma-buf support");
            }
        }
        Ok(())
    }

    fn remove_gpu(&mut self, id: u64, state: &mut State) {
        if state.dmabuf.device == Some(id) { state.disable_dmabuf(); }
        if let Some(gpu) = self.gpus.remove(&id) {
            self.handle.remove(gpu.token);
            for surface in gpu.surfaces.values() {
                surface.disconnect(state, &self.handle);
            }
        }
    }

    fn rescan(&mut self, state: &mut State) {
        if !self.active {
            return;
        }
        for gpu in self.gpus.values_mut() {
            if let Err(err) = gpu.scan(state, &self.handle) {
                tracing::warn!(?err, "connector scan failed");
            }
        }
    }

    fn render(&mut self, state: &mut State) {
        let now = Instant::now();
        state.update_space(now);
        if let Some(gpu) = state.dmabuf.device.and_then(|id| self.gpus.get_mut(&id)) {
            state.dmabuf.process_imports(|dmabuf| gpu.renderer.import_dmabuf(dmabuf, None).is_ok());
        }
        for gpu in self.gpus.values_mut() {
            for surface in gpu.surfaces.values_mut() {
                // Draw whenever no flip is outstanding and its time has come
                // (`draw_at`, while flipping): on a commit, input, that time,
                // and at least once a refresh while something animates (the
                // poll).
                if surface.pending || surface.draw_at.is_some_and(|at| now < at) {
                    continue;
                }
                surface.draw_at = None;
                let started = Instant::now();
                let output = state.screens[&surface.screen].output.clone();
                let refresh = output.current_mode().map_or(60_000, |mode| mode.refresh).max(1);
                surface.next_frame = now + Duration::from_secs_f64(1000.0 / refresh as f64);
                let elements = state.screen_elements(&mut gpu.renderer, surface.screen, now);
                let frame = match surface.compositor.render_frame(&mut gpu.renderer, &elements, BACKGROUND, FrameFlags::empty()) {
                    Ok(frame) => frame,
                    Err(err) => {
                        tracing::warn!(?err, "DRM render failed");
                        continue;
                    }
                };
                if frame.needs_sync()
                    && let PrimaryPlaneElement::Swapchain(element) = frame.primary_element
                    && let Err(err) = element.sync.wait()
                {
                    tracing::warn!(?err, "render synchronization failed");
                    continue;
                }
                if frame.is_empty {
                    // Nothing new to show: the commits so far changed nothing
                    // here, so they don't count towards commit-to-screen time.
                    state.committed_at = None;
                    continue;
                }
                match surface.compositor.queue_frame(()) {
                    Ok(()) => {
                        surface.pending = true;
                        surface.committed_at = state.committed_at.take();
                        surface.queued_at = Some(Instant::now());
                        surface.clock.drew(started.elapsed());
                        tracing::trace!(screen = surface.screen.0, "DRM frame queued");
                    }
                    Err(err) => tracing::warn!(?err, "DRM submit failed"),
                }
            }
        }
        // Screenshots, on the GPU that clients' buffers are on.
        let id = state.dmabuf.device.filter(|id| self.gpus.contains_key(id)).or_else(|| self.gpus.keys().next().copied());
        match id.and_then(|id| self.gpus.get_mut(&id)) {
            Some(gpu) => state.take_shots(&mut gpu.renderer),
            None => state.fail_shots("no GPU to draw with"),
        }
        // Screens with flips get callbacks from them (the VBlank event). The
        // poll covers the rest at the refresh: windows no screen shows, and
        // screens with nothing new to flip (an app's first frame, or an
        // unchanged one), even after the last monitor is unplugged.
        if now >= self.callbacks_at {
            let surfaces = || self.gpus.values().flat_map(|gpu| gpu.surfaces.values());
            let period = surfaces()
                .filter_map(|surface| state.screens.get(&surface.screen)?.output.current_mode())
                .map(|mode| Duration::from_secs_f64(1000.0 / mode.refresh.max(1) as f64))
                .min()
                .unwrap_or(Duration::from_millis(16));
            let flipping: Vec<ScreenId> = surfaces()
                .filter(|surface| surface.flipped_at.is_some_and(|at| now.duration_since(at) < period * 3 / 2))
                .map(|surface| surface.screen)
                .collect();
            self.callbacks_at = now + period;
            state.send_frames(&self.callback_output, |screen| screen.is_none_or(|id| !flipping.contains(&id)));
        }
        state.after_frame();
    }
}

/// What `Gpu::names` finds out about a connector.
struct Names {
    name: String,
    connector_id: String,
    identity: String,
    make: String,
    model: String,
    serial: String,
}

impl Gpu {
    fn scan(&mut self, state: &mut State, handle: &LoopHandle<'static, State>) -> Result<(), Error> {
        self.scanner.scan_connectors(&self.drm)?;
        // Reconcile the full mapping: a hotplug can reassign CRTCs, and an
        // earlier connector initialization may have failed transiently.
        let connectors: Vec<_> = self
            .scanner
            .crtcs()
            .map(|(info, crtc)| (info.clone(), crtc))
            .filter(|(info, _)| {
                let names = self.names(info);
                !state.screen_off(&names.name, &names.identity)
            })
            .collect();
        let gone: Vec<_> = self
            .surfaces
            .iter()
            .filter(|(crtc, surface)| {
                !connectors
                    .iter()
                    .any(|(info, mapped)| mapped == *crtc && info.handle() == surface.connector)
            })
            .map(|(&crtc, _)| crtc)
            .collect();
        for crtc in gone {
            if let Some(mut surface) = self.surfaces.remove(&crtc) {
                // Blank it (turned off, the monitor is still there).
                if let Err(err) = surface.compositor.clear() {
                    tracing::debug!(?err, "couldn't blank a screen going away");
                }
                surface.disconnect(state, handle);
            }
        }
        self.needs_scan = false;
        for (connector, crtc) in connectors {
            if let Err(err) = self.connect(connector, crtc, state) {
                tracing::warn!(?err, "output unavailable");
            }
        }
        self.blank_unused();
        Ok(())
    }

    /// Turn off what still shows on screens Misu doesn't use (the device is
    /// opened keeping lit screens on, `add_gpu`): the console's or an earlier
    /// session's picture on one that is off in the config. A surface that is
    /// dropped clears its CRTC.
    fn blank_unused(&mut self) {
        let Ok(resources) = self.drm.resource_handles() else { return };
        for &crtc in resources.crtcs() {
            if self.surfaces.contains_key(&crtc) {
                continue;
            }
            let Some(mode) = self.drm.get_crtc(crtc).ok().and_then(|info| info.mode()) else { continue };
            let lit: Vec<_> = resources
                .connectors()
                .iter()
                .copied()
                .filter(|&connector| {
                    let encoder = self.drm.get_connector(connector, false).ok().and_then(|info| info.current_encoder());
                    encoder.and_then(|encoder| self.drm.get_encoder(encoder).ok()).and_then(|info| info.crtc()) == Some(crtc)
                })
                .collect();
            match self.drm.create_surface(crtc, mode, &lit) {
                Ok(surface) => {
                    tracing::info!(?crtc, "blanking a screen Misu doesn't use");
                    drop(surface);
                }
                Err(err) => tracing::warn!(?err, ?crtc, "couldn't blank an unused screen"),
            }
        }
    }

    /// A connector's output name (`card1-eDP-1`), its stable id, its
    /// monitor's make, model and serial and identity (as `state::identity`
    /// makes it, alone).
    fn names(&self, connector: &connector::Info) -> Names {
        let (kind, n) = (connector.interface().as_str(), connector.interface_id());
        let info = display_info::for_connector(&self.drm, connector.handle());
        let make = info.as_ref().and_then(|i| i.make()).unwrap_or_default();
        let model = info.as_ref().and_then(|i| i.model()).unwrap_or_default();
        let serial = info.as_ref().and_then(|i| i.serial()).unwrap_or_default();
        let connector_id = format!("{}-{kind}-{n}", self.identity);
        let identity = crate::state::identity(&make, &model, &serial, &connector_id, false);
        Names { name: format!("{}-{kind}-{n}", self.name), connector_id, identity, make, model, serial }
    }

    fn connect(&mut self, connector: connector::Info, crtc: crtc::Handle, state: &mut State) -> Result<(), Error> {
        let preferred = connector
            .modes()
            .iter()
            .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
            .or_else(|| connector.modes().first())
            .copied()
            .ok_or("connector has no mode")?;
        let names = self.names(&connector);
        let wanted = crate::screens::find(&state.config.screens, &names.name, &names.identity).and_then(|i| state.config.screens[i].mode);
        let modes: Vec<(i32, i32, i32)> = connector
            .modes()
            .iter()
            .map(|m| {
                let m = Mode::from(*m);
                (m.size.w, m.size.h, m.refresh)
            })
            .collect();
        let chosen = wanted.and_then(|wanted| {
            let index = crate::screens::choose_mode(&modes, wanted);
            if index.is_none() {
                tracing::warn!(screen = %names.name, "no mode {}x{}: using the preferred one", wanted.0, wanted.1);
            }
            index
        });
        let mode = chosen.map_or(preferred, |i| connector.modes()[i]);
        let wl_mode = Mode::from(mode);
        if let Some(surface) = self.surfaces.get_mut(&crtc) {
            if surface.compositor.pending_mode() == mode {
                return Ok(());
            }
            if surface.pending {
                self.needs_scan = true;
                return Ok(());
            }
            surface.compositor.use_mode(mode)?;
            let output = &state.screens[&surface.screen].output;
            output.change_current_state(Some(wl_mode), None, None, None);
            output.set_preferred(wl_mode);
            state.resize_screen(surface.screen);
            return Ok(());
        }
        let output = Output::new(
            names.name,
            PhysicalProperties {
                size: connector.size().map_or((0, 0), |(w, h)| (w as i32, h as i32)).into(),
                subpixel: Subpixel::Unknown,
                make: names.make,
                model: names.model,
                serial_number: names.serial,
            },
        );
        output.change_current_state(Some(wl_mode), None, None, Some((0, 0).into()));
        output.user_data().insert_if_missing(|| ConnectorId(names.connector_id));
        output.set_preferred(wl_mode);
        for mode in connector.modes() {
            output.add_mode(Mode::from(*mode));
        }
        let started = Instant::now();
        let surface = self.drm.create_surface(crtc, mode, &[connector.handle()])?;
        let created = started.elapsed();
        let compositor = DrmCompositor::new(
            &output,
            surface,
            None,
            GbmAllocator::new(self.gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT),
            GbmFramebufferExporter::new(self.gbm.clone(), None.into()),
            [Fourcc::Argb8888, Fourcc::Xrgb8888],
            self.renderer.dmabuf_formats(),
            self.drm.cursor_size(),
            None,
        )?;
        tracing::info!(surface = ?created, ready = ?started.elapsed(), "screen {} set up", output.name());
        let global = output.create_global::<State>(&state.display_handle);
        let screen = state.add_screen(output);
        self.surfaces.insert(
            crtc,
            Surface {
                screen,
                connector: connector.handle(),
                global,
                compositor,
                pending: false,
                next_frame: Instant::now(),
                committed_at: None,
                sequence: None,
                flipped_at: None,
                queued_at: None,
                draw_at: None,
                clock: Default::default(),
            },
        );
        Ok(())
    }
}
impl Surface {
    fn disconnect(&self, state: &mut State, handle: &LoopHandle<'static, State>) {
        state.disconnect(self.screen);
        let global = self.global.clone();
        state.display_handle.disable_global::<State>(global.clone());
        if let Err(err) = handle.insert_source(Timer::from_duration(Duration::from_secs(10)), move |_, _, state| {
            state.display_handle.remove_global::<State>(global.clone());
            TimeoutAction::Drop
        }) {
            tracing::warn!(?err, "output global cleanup timer failed");
        }
    }
}
