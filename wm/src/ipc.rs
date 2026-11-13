//! The control socket: one command per connection, one JSON line back.
//!
//! ```text
//! wm msg tree
//! wm msg focus right
//! wm msg seat 1 click 300 200
//! ```
//!
//! It is `$XDG_RUNTIME_DIR/wm.<wayland socket>.sock`, and its path is in
//! `$WM_SOCK` for everything started inside the compositor, which `wm msg`
//! reads. The server never reads `WM_SOCK`: a wm started inside another
//! would take the outer one's socket.
//!
//! `wm msg subscribe` keeps its connection open, for a bar or a script that
//! waits. After `{"ok":true}` it gets the current state as `new` events, then
//! one JSON line for each change, whoever made it (a key, another seat, an
//! app, `wm msg`):
//!
//! ```text
//! {"event":"seat","change":"change","id":1,"workspace":4,"number":2,"focus":7,…}
//! {"event":"window","change":"new","id":9,"app_id":"kitty","title":"~","workspace":4}
//! {"event":"window","change":"gone","id":9}
//! {"event":"screen","change":"change","id":2,"connected":false,…}
//! ```
//!
//! After each turn of the event loop, the state is summed up and compared
//! with the last summary, so no change can be missed and none is announced
//! twice. A subscriber that doesn't keep up (its socket buffer full) is
//! dropped.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use smithay::{
    backend::input::{ButtonState, InputTime},
    desktop::{Window, layer_map_for_output},
    reexports::{
        calloop::{EventLoop, Interest, Mode, PostAction, generic::Generic},
        wayland_protocols::xdg::shell::server::xdg_toplevel::State::Activated,
    },
    wayland::compositor::get_parent,
};

use crate::{
    input::{Action, BTN_LEFT},
    layout::Direction,
    shot::Shot,
    state::{FALLBACK_SIZE, Screen, ScreenId, State, WorkspaceId, new_output, window_id, window_info},
};

pub const ENV: &str = "WM_SOCK";

pub const HELP: &str = "[seat N] COMMAND, where COMMAND is one of:
  tree                      windows, workspaces and seats, as JSON
  subscribe                 keep the connection: the state, then a JSON
                            line for each change (see src/ipc.rs)
  spawn CMD…                run a command
  close                     close the seat's focused window
  focus left|right          focus the column to the side
  focus first|last          focus the first or last column
  move left|right           move the focused column
  workspace N               go to workspace N (1-9)
  move-to-workspace N [follow]
                            send the focused window there (and go with it)
  animations on|off|toggle  for the seat (they show while it's on screen)
  pointer X Y               move the seat's pointer
  click [X Y]               left click, after moving there if given
  next-seat                 drive the next seat with the host's input
  reload                    read the config file again
  screenshot FILE           the seat's view as a PNG, on screen or not
  take-workspace ID         bring a workspace (a floating one, say) to the
                            seat's screen, and go there
  screen-new                add a virtual screen (nothing to see; for tests)
  screen-remove N           unplug screen N: it floats, keeping its workspaces
  screen-add N              plug it back
  quit
`seat N` (from 0) picks the seat; without it, the one the host drives.";

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Tree,
    Help,
    /// Keep the connection open for events.
    Subscribe,
    Action(Action),
    Pointer(f64, f64),
    Click(Option<(f64, f64)>),
    TakeWorkspace(u64),
    /// A virtual screen (no picture), for tests and off-screen work.
    ScreenNew,
    /// The seat's view, as a PNG, taken with the next frame.
    Screenshot(PathBuf),
    /// Unplug and plug back, as if the monitor were.
    ScreenRemove(u32),
    ScreenAdd(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub seat: Option<usize>,
    pub command: Command,
}

pub fn parse(line: &str) -> Result<Request, String> {
    let mut words: Vec<&str> = line.split_whitespace().collect();
    let mut seat = None;
    if words.first() == Some(&"seat") {
        let n = words.get(1).ok_or("seat needs a number")?;
        seat = Some(n.parse().map_err(|_| format!("bad seat {n}"))?);
        words.drain(..2);
    }
    let (&name, args) = words.split_first().ok_or("no command")?;
    let number = |s: &str| s.parse::<f64>().map_err(|_| format!("bad number {s}"));
    let direction = |args: &[&str]| match args {
        ["left"] => Ok(Direction::Left),
        ["right"] => Ok(Direction::Right),
        _ => Err(format!("{name} takes left or right")),
    };
    let workspace = |args: &[&str]| match args {
        [n] => match n.parse::<usize>() {
            Ok(n @ 1..=9) => Ok(n - 1),
            _ => Err(format!("{name} takes 1-9")),
        },
        _ => Err(format!("{name} takes a workspace number")),
    };
    let command = match name {
        "tree" if args.is_empty() => Command::Tree,
        "subscribe" if args.is_empty() => Command::Subscribe,
        "help" => Command::Help,
        "spawn" if !args.is_empty() => Command::Action(Action::Spawn(args.join(" "))),
        "close" => Command::Action(Action::Close),
        "focus" => Command::Action(match args {
            ["first"] => Action::FocusEnd(false),
            ["last"] => Action::FocusEnd(true),
            _ => Action::Focus(direction(args).map_err(|_| "focus takes left, right, first or last".to_string())?),
        }),
        "move" => Command::Action(Action::Move(direction(args)?)),
        "workspace" => Command::Action(Action::Workspace(workspace(args)?)),
        "move-to-workspace" => Command::Action(match args {
            [n, "follow"] => Action::MoveToWorkspace(workspace(&[n])?, true),
            _ => Action::MoveToWorkspace(workspace(args)?, false),
        }),
        "animations" => Command::Action(Action::Animations(match args {
            ["on"] => Some(true),
            ["off"] => Some(false),
            ["toggle"] | [] => None,
            _ => return Err("animations takes on, off or toggle".into()),
        })),
        "pointer" => match args {
            [x, y] => Command::Pointer(number(x)?, number(y)?),
            _ => return Err("pointer takes X Y".into()),
        },
        "click" => match args {
            [] => Command::Click(None),
            [x, y] => Command::Click(Some((number(x)?, number(y)?))),
            _ => return Err("click takes nothing or X Y".into()),
        },
        "next-seat" => Command::Action(Action::NextSeat),
        "reload" if args.is_empty() => Command::Action(Action::Reload),
        "take-workspace" => match args {
            [id] => Command::TakeWorkspace(id.parse().map_err(|_| format!("bad workspace id {id}"))?),
            _ => return Err("take-workspace takes a workspace id".into()),
        },
        "screenshot" => match args {
            // `wm msg` makes it absolute: wm's working directory isn't the caller's.
            [path] if Path::new(path).is_absolute() => Command::Screenshot(PathBuf::from(path)),
            [_] => return Err("screenshot takes an absolute path".into()),
            _ => return Err("screenshot takes a file".into()),
        },
        "screen-new" if args.is_empty() => Command::ScreenNew,
        "screen-remove" | "screen-add" => {
            let [id] = args else { return Err(format!("{name} takes a screen number")) };
            let id = id.parse().map_err(|_| format!("bad screen {id}"))?;
            if name == "screen-remove" { Command::ScreenRemove(id) } else { Command::ScreenAdd(id) }
        }
        "quit" => Command::Action(Action::Quit),
        _ => return Err(format!("unknown command: {}", words.join(" "))),
    };
    Ok(Request { seat, command })
}

/// How long a client has to send its command, and to take the reply.
pub const TIMEOUT: Duration = Duration::from_millis(500);
/// The longest command line.
pub const MAX_LINE: usize = 4096;

/// The socket's path, named after the Wayland socket.
fn path(wayland_socket: &OsStr) -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    dir.join(format!("wm.{}.sock", wayland_socket.to_string_lossy()))
}

pub fn listen(event_loop: &mut EventLoop<State>, wayland_socket: &OsStr) -> std::io::Result<PathBuf> {
    let path = path(wayland_socket);
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;
    event_loop
        .handle()
        .insert_source(Generic::new(listener, Interest::READ, Mode::Level), |_, listener, state| {
            while let Ok((stream, _)) = listener.accept() {
                if let Err(err) = serve(state, stream) {
                    tracing::debug!("control socket: {err}");
                }
            }
            Ok(PostAction::Continue)
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(path)
}

/// One command (or a subscription, kept). The compositor waits for it, so a
/// client gets `TIMEOUT` in all to send it and as long to take the reply.
fn serve(state: &mut State, mut stream: UnixStream) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    let line = read_request(&stream)?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let reply = match parse(&line) {
        Ok(Request { command: Command::Subscribe, .. }) => {
            writeln!(stream, "{}", json!({ "ok": true }))?;
            state.subscribe(stream);
            return Ok(());
        }
        // The reply comes with the next frame.
        Ok(Request { seat, command: Command::Screenshot(path) }) => match state.seat_index(seat) {
            Ok(seat) => {
                stream.set_write_timeout(Some(TIMEOUT))?;
                state.shots.push(Shot { seat, path, stream });
                return Ok(());
            }
            Err(error) => Err(error),
        },
        Ok(request) => state.handle_request(request),
        Err(error) => Err(error),
    };
    let reply = match reply {
        Ok(Value::Null) => json!({ "ok": true }),
        Ok(value) => json!({ "ok": true, "result": value }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
    writeln!(stream, "{reply}")
}

/// A line from `stream`, within `TIMEOUT` overall and `MAX_LINE` bytes.
pub fn read_request(mut stream: &UnixStream) -> std::io::Result<String> {
    let deadline = Instant::now() + TIMEOUT;
    let mut line = Vec::new();
    let mut buf = [0; 512];
    while !line.contains(&b'\n') {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        stream.set_read_timeout(Some(left))?;
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        line.extend_from_slice(&buf[..n]);
        if line.len() > MAX_LINE {
            return Err(std::io::Error::other("command too long"));
        }
    }
    let end = line.iter().position(|&b| b == b'\n').unwrap_or(line.len());
    String::from_utf8(line[..end].to_vec()).map_err(std::io::Error::other)
}

/// `wm msg …`: returns the exit code.
pub fn client(words: &[String]) -> i32 {
    if words.is_empty() || words[0] == "help" {
        println!("{HELP}");
        return if words.is_empty() { 2 } else { 0 };
    }
    let Some(path) = std::env::var_os(ENV) else {
        eprintln!("${ENV} isn't set: run this inside wm, or set it to its control socket");
        return 2;
    };
    // Every line until wm closes: one reply, or with `subscribe`, events.
    let result = (|| -> std::io::Result<bool> {
        let mut stream = UnixStream::connect(&path)?;
        let mut words = words.to_vec();
        // A screenshot's file, from where `wm msg` runs.
        if let Some(i) = words.iter().position(|w| w == "screenshot")
            && let Some(file) = words.get_mut(i + 1)
            && let Ok(absolute) = std::path::absolute(&*file)
        {
            *file = absolute.to_string_lossy().into_owned();
        }
        writeln!(stream, "{}", words.join(" "))?;
        let mut ok = false;
        let mut out = std::io::stdout();
        for (i, line) in BufReader::new(stream).lines().enumerate() {
            let line = line?;
            if i == 0 {
                ok = serde_json::from_str::<Value>(&line).is_ok_and(|v| v["ok"] == true);
            }
            writeln!(out, "{line}")?;
            out.flush()?;
        }
        Ok(ok)
    })();
    match result {
        Ok(ok) => if ok { 0 } else { 1 },
        Err(err) => {
            eprintln!("{}: {err}", PathBuf::from(path).display());
            1
        }
    }
}

impl State {
    /// The seat a request names, else the one the host drives.
    pub fn seat_index(&self, seat: Option<usize>) -> Result<usize, String> {
        let seat = seat.unwrap_or(self.seats.active);
        if seat >= self.seats.list.len() {
            return Err(format!("no seat {seat}: there are {}", self.seats.list.len()));
        }
        Ok(seat)
    }

    pub fn handle_request(&mut self, request: Request) -> Result<Value, String> {
        let seat = self.seat_index(request.seat)?;
        match request.command {
            Command::Tree => return Ok(self.tree()),
            Command::Help => return Ok(Value::String(HELP.into())),
            Command::Subscribe => return Err("subscribe takes a connection of its own".into()),
            Command::Screenshot(_) => return Err("screenshot replies with a frame, over a connection of its own".into()),
            Command::Action(action) => self.run(seat, action),
            Command::Pointer(x, y) => self.pointer_motion(seat, (x, y).into(), InputTime::now()),
            Command::TakeWorkspace(id) => self.take_workspace(seat, WorkspaceId(id))?,
            Command::ScreenNew => {
                // Make `wm`, model `virtual`, and its number as the serial.
                let virtuals = self.screens.values().filter(|s| s.output.physical_properties().model == "virtual");
                let n = 1 + virtuals.count();
                let output = new_output(&format!("VIRTUAL-{n}"), "virtual", &n.to_string(), FALLBACK_SIZE.into());
                return Ok(json!(self.add_screen(output).0));
            }
            Command::ScreenRemove(id) | Command::ScreenAdd(id) if !self.screens.contains_key(&ScreenId(id)) => {
                return Err(format!("no screen {id}"));
            }
            Command::ScreenRemove(id) => self.disconnect(ScreenId(id)),
            Command::ScreenAdd(id) => self.connect(ScreenId(id)),
            Command::Click(at) => {
                if let Some((x, y)) = at {
                    self.pointer_motion(seat, (x, y).into(), InputTime::now());
                }
                self.pointer_button(seat, BTN_LEFT, ButtonState::Pressed, InputTime::now());
                self.pointer_button(seat, BTN_LEFT, ButtonState::Released, InputTime::now());
            }
        }
        Ok(Value::Null)
    }

    /// A workspace's number on its screen, from 1.
    fn number(&self, ws: WorkspaceId) -> Option<usize> {
        self.screen_of(ws).and_then(|s| self.screens[&s].number_of(ws))
    }

    // What subscribers hear of a seat, a screen and a window: nothing that
    // moves all the time (pointers, views mid-animation). `tree` adds those.

    fn seat_json(&self, index: usize) -> Value {
        let s = &self.seats[index];
        json!({
            "id": index, "name": s.seat.name(), "active": index == self.seats.active,
            "shown": self.is_shown(index), "animate": s.animate,
            "workspace": s.workspace.0, "screen": self.screen_of(s.workspace).map(|s| s.0),
            "number": self.number(s.workspace), "focus": s.focus.as_ref().map(window_id),
        })
    }

    fn screen_json(&self, s: &Screen) -> Value {
        let position = self.connected.iter().position(|&c| c == s.id);
        json!({
            "id": s.id.0, "name": s.output.name(), "identity": s.identity,
            "connected": position.is_some(), "position": position,
            "seat": s.viewer, "active": self.shows(s.id).0,
            "workspaces": s.workspaces.iter().map(|w| w.0).collect::<Vec<_>>(),
        })
    }

    fn window_json(&self, window: &Window, ws: WorkspaceId) -> Value {
        let (app_id, title) = window_info(window);
        json!({ "id": window_id(window), "app_id": app_id, "title": title, "workspace": ws.0, "number": self.number(ws) })
    }

    /// The seats, screens and windows, for `diff`.
    pub fn snapshot(&self) -> Snapshot {
        let seats = (0..self.seats.list.len()).map(|i| (("seat", i as u64), self.seat_json(i)));
        let screens = self.screens.values().map(|s| (("screen", u64::from(s.id.0)), self.screen_json(s)));
        let windows = self.workspaces.iter().flat_map(|(&id, ws)| {
            ws.windows().map(move |w| (("window", window_id(w)), self.window_json(w, id)))
        });
        seats.chain(screens).chain(windows).collect()
    }

    /// Everything a test or a bar wants to know, as JSON: what subscribers
    /// hear and more. Window rectangles are where they are headed along
    /// their workspace; a seat sees them shifted left by its `view` there.
    pub fn tree(&self) -> Value {
        let now = Instant::now();
        let mut screens: Vec<&Screen> = self.screens.values().collect();
        screens.sort_by_key(|s| s.id);
        let screens: Vec<Value> = screens
            .into_iter()
            .map(|s| {
                let (size, p, area) = (s.size(), s.output.physical_properties(), s.area());
                let map = layer_map_for_output(&s.output);
                let layers: Vec<Value> = map
                    .layers()
                    .map(|l| {
                        let r = map.layer_geometry(l).unwrap_or_default();
                        json!({
                            "namespace": l.namespace(), "layer": format!("{:?}", l.layer()).to_lowercase(),
                            "x": r.loc.x, "y": r.loc.y, "width": r.size.w, "height": r.size.h,
                        })
                    })
                    .collect();
                let more = json!({
                    "make": p.make, "model": p.model, "serial": p.serial_number,
                    "width": size.w, "height": size.h,
                    // What its workspaces get: all but what layers keep.
                    "area": { "x": area.loc.x, "y": area.loc.y, "width": area.size.w, "height": area.size.h },
                    "layers": layers,
                    // What is drawn on it.
                    "windows": s.space.elements_for_output(&s.output).map(window_id).collect::<Vec<_>>(),
                });
                with(self.screen_json(s), more)
            })
            .collect();
        let mut ids: Vec<WorkspaceId> = self.workspaces.keys().copied().collect();
        ids.sort_unstable();
        let workspaces: Vec<Value> = ids
            .into_iter()
            .map(|id| {
                let screen = self.screen_of(id);
                let windows: Vec<Value> = self.workspaces[&id]
                    .targets()
                    .map(|(w, r)| {
                        let more = json!({
                            "x": r.loc.x, "y": r.loc.y, "width": r.size.w, "height": r.size.h,
                            // Over its parent (a dialog), at its own size.
                            "floating": self.workspaces[&id].is_floating(w),
                            "configured": w.toplevel().is_some_and(|t| t.is_initial_configure_sent()),
                            "activated": w.toplevel().is_some_and(|t| t.with_pending_state(|s| s.states.contains(Activated))),
                        });
                        with(self.window_json(w, id), more)
                    })
                    .collect();
                json!({
                    "id": id.0,
                    "screen": screen.map(|s| s.0),
                    "floating": screen.is_none_or(|s| !self.connected.contains(&s)),
                    "number": self.number(id),
                    "windows": windows,
                })
            })
            .collect();
        let seats: Vec<Value> = (0..self.seats.list.len())
            .map(|i| {
                let s = &self.seats[i];
                let pointer = s.seat.get_pointer().unwrap();
                let p = pointer.current_location();
                // The window its pointer events go to.
                let over = pointer.current_focus().and_then(|mut surface| {
                    while let Some(parent) = get_parent(&surface) {
                        surface = parent;
                    }
                    self.window_for_surface(&surface).as_ref().map(window_id)
                });
                let ws = &self.workspaces[&s.workspace];
                let more = json!({
                    "index": i, "workspace_number": self.number(s.workspace),
                    "view": ws.view_target(i), "animating": ws.is_animating(i, now),
                    "pointer": { "x": p.x, "y": p.y, "over": over },
                    // The layer surface with its keyboard, by namespace.
                    "layer_focus": s.layer_focus.as_ref().map(|l| l.namespace().to_owned()),
                });
                with(self.seat_json(i), more)
            })
            .collect();
        json!({
            "wayland_display": self.socket_name.to_string_lossy(),
            "screens": screens,
            "workspaces": workspaces,
            "seats": seats,
        })
    }

    /// Keep `stream` told about changes, starting with how things are now.
    pub fn subscribe(&mut self, mut stream: UnixStream) {
        self.publish();
        let now = self.snapshot();
        if stream.set_nonblocking(true).is_ok() && send(&mut stream, &diff(&Snapshot::new(), &now)) {
            self.events.subscribers.push(stream);
        }
        self.events.last = now;
    }

    /// Tell the subscribers what changed since the last time.
    pub fn publish(&mut self) {
        if self.events.subscribers.is_empty() {
            return;
        }
        let now = self.snapshot();
        let events = diff(&self.events.last, &now);
        if !events.is_empty() {
            self.events.subscribers.retain_mut(|s| send(s, &events));
        }
        self.events.last = now;
    }
}

/// `base` (an object) with `more`'s fields too.
fn with(mut base: Value, more: Value) -> Value {
    if let (Some(base), Value::Object(more)) = (base.as_object_mut(), more) {
        base.extend(more);
    }
    base
}

/// What a subscriber is told about, keyed by kind and id: each value is an
/// object, and a change to any of its fields is an event.
pub type Snapshot = BTreeMap<(&'static str, u64), Value>;

/// Who `wm msg subscribe`s, and what they were last told.
#[derive(Default)]
pub struct Events {
    subscribers: Vec<UnixStream>,
    last: Snapshot,
}

/// The events that turn `old` into `new`: `new` and `change` carry all of the
/// thing's fields, `gone` only its id.
pub fn diff(old: &Snapshot, new: &Snapshot) -> Vec<Value> {
    let mut events = Vec::new();
    for (&(kind, id), value) in new {
        let change = match old.get(&(kind, id)) {
            Some(before) if before == value => continue,
            Some(_) => "change",
            None => "new",
        };
        events.push(with(json!({ "event": kind, "change": change }), value.clone()));
    }
    for &(kind, id) in old.keys().filter(|key| !new.contains_key(key)) {
        events.push(json!({ "event": kind, "change": "gone", "id": id }));
    }
    events
}

/// Whether all of `events` went out.
fn send(stream: &mut UnixStream, events: &[Value]) -> bool {
    let text: String = events.iter().map(|e| format!("{e}\n")).collect();
    stream.write_all(text.as_bytes()).is_ok()
}
