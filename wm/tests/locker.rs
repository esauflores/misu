//! A session locker for the e2e run: locks wm through `ext-session-lock`,
//! covers each screen in one colour, and notes which seats' keyboards come
//! to it; then unlocks, or dies without unlocking.

use std::{
    fs::File,
    io::Write,
    os::{fd::AsFd, unix::net::UnixStream},
    path::{Path, PathBuf},
    thread::sleep,
    time::{Duration, Instant},
};

use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_buffer::WlBuffer, wl_compositor::WlCompositor, wl_keyboard::{self, WlKeyboard}, wl_output::WlOutput,
        wl_registry::WlRegistry, wl_seat::WlSeat, wl_shm::{self, WlShm}, wl_shm_pool::WlShmPool, wl_surface::WlSurface,
    },
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1::ExtSessionLockManagerV1,
    ext_session_lock_surface_v1::{self, ExtSessionLockSurfaceV1},
    ext_session_lock_v1::{self, ExtSessionLockV1},
};

#[derive(Default)]
pub struct Seen {
    pub locked: bool,
    /// Refused (another locker holds the lock) or ended.
    pub finished: bool,
    /// Lock surfaces drawn.
    pub drawn: usize,
    /// The seats (by the order wm lists them) whose keyboard is on a lock
    /// surface now.
    pub keyboards: Vec<usize>,
}

struct Data {
    seen: Seen,
    shm: WlShm,
    color: [u8; 3],
    dir: PathBuf,
    surfaces: Vec<WlSurface>,
}

pub struct Locker {
    _conn: Connection,
    queue: EventQueue<Data>,
    data: Data,
    lock: ExtSessionLockV1,
}

impl Locker {
    /// Ask wm on `display` (in `runtime`) to lock; screens go `color`.
    pub fn lock(runtime: &Path, display: &str, color: [u8; 3], dir: &Path) -> Self {
        let conn = Connection::from_socket(UnixStream::connect(runtime.join(display)).unwrap()).unwrap();
        let (globals, queue) = registry_queue_init::<Data>(&conn).unwrap();
        let qh = queue.handle();
        let compositor: WlCompositor = globals.bind(&qh, 4..=6, ()).unwrap();
        let shm: WlShm = globals.bind(&qh, 1..=1, ()).unwrap();
        let manager: ExtSessionLockManagerV1 = globals.bind(&qh, 1..=1, ()).unwrap();
        let list = globals.contents().clone_list();
        let mut seats = 0;
        for global in &list {
            match global.interface.as_str() {
                "wl_seat" => {
                    let seat: WlSeat = globals.registry().bind(global.name, 5, &qh, seats);
                    seat.get_keyboard(&qh, seats);
                    seats += 1;
                }
                "wl_output" => {}
                _ => {}
            }
        }
        let lock = manager.lock(&qh, ());
        let mut surfaces = Vec::new();
        for global in list.iter().filter(|g| g.interface == "wl_output") {
            let output: WlOutput = globals.registry().bind(global.name, 1, &qh, ());
            let surface = compositor.create_surface(&qh, ());
            lock.get_lock_surface(&surface, &output, &qh, surface.clone());
            surfaces.push(surface);
        }
        let data = Data { seen: Seen::default(), shm, color, dir: dir.to_path_buf(), surfaces };
        Self { _conn: conn, queue, data, lock }
    }

    /// Wait (15 s at most) until `done` says so.
    pub fn wait(&mut self, what: &str, done: impl Fn(&Seen) -> bool) {
        let start = Instant::now();
        loop {
            self.queue.roundtrip(&mut self.data).unwrap();
            if done(&self.data.seen) {
                return;
            }
            assert!(start.elapsed() < Duration::from_secs(15), "timed out waiting for {what}");
            sleep(Duration::from_millis(50));
        }
    }

    pub fn seen(&self) -> &Seen {
        &self.data.seen
    }

    pub fn unlock(mut self) {
        self.lock.unlock_and_destroy();
        self.queue.roundtrip(&mut self.data).unwrap();
    }

    /// Go away without unlocking, as a crashed locker would.
    pub fn crash(self) {}
}

impl Dispatch<ExtSessionLockV1, ()> for Data {
    fn event(data: &mut Self, _: &ExtSessionLockV1, event: ext_session_lock_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            ext_session_lock_v1::Event::Locked => data.seen.locked = true,
            ext_session_lock_v1::Event::Finished => data.seen.finished = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtSessionLockSurfaceV1, WlSurface> for Data {
    fn event(
        data: &mut Self,
        lock_surface: &ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event,
        surface: &WlSurface,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let ext_session_lock_surface_v1::Event::Configure { serial, width, height } = event else { return };
        lock_surface.ack_configure(serial);
        let (w, h) = (width as i32, height as i32);
        let path = data.dir.join(format!("lock-{}.shm", data.seen.drawn));
        let mut file = File::options().read(true).write(true).create(true).truncate(true).open(&path).unwrap();
        let [r, g, b] = data.color;
        let pixel = [b, g, r, 255];
        let pixels: Vec<u8> = pixel.iter().copied().cycle().take((w * h * 4) as usize).collect();
        file.write_all(&pixels).unwrap();
        let pool = data.shm.create_pool(file.as_fd(), w * h * 4, qh, ());
        let buffer = pool.create_buffer(0, w, h, w * 4, wl_shm::Format::Argb8888, qh, ());
        pool.destroy();
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, w, h);
        surface.commit();
        data.seen.drawn += 1;
    }
}

impl Dispatch<WlKeyboard, usize> for Data {
    fn event(data: &mut Self, _: &WlKeyboard, event: wl_keyboard::Event, seat: &usize, _: &Connection, _: &QueueHandle<Self>) {
        match event {
            wl_keyboard::Event::Enter { surface, .. } if data.surfaces.contains(&surface) => {
                if !data.seen.keyboards.contains(seat) {
                    data.seen.keyboards.push(*seat);
                }
            }
            wl_keyboard::Event::Leave { .. } => data.seen.keyboards.retain(|s| s != seat),
            _ => {}
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for Data {
    fn event(_: &mut Self, _: &WlRegistry, _: wayland_client::protocol::wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlSeat, usize> for Data {
    fn event(_: &mut Self, _: &WlSeat, _: wayland_client::protocol::wl_seat::Event, _: &usize, _: &Connection, _: &QueueHandle<Self>) {}
}

delegate_noop!(Data: ignore WlCompositor);
delegate_noop!(Data: ignore WlShm);
delegate_noop!(Data: ignore WlShmPool);
delegate_noop!(Data: ignore WlBuffer);
delegate_noop!(Data: ignore WlOutput);
delegate_noop!(Data: ignore WlSurface);
delegate_noop!(Data: ignore ExtSessionLockManagerV1);
