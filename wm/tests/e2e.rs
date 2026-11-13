//! End to end: wm nested in a headless scroll (nothing shows on screen),
//! with kitty windows, driven and checked through the control socket.
//!
//! Needs `scroll` and `kitty` on PATH (skipped otherwise) and a GPU. Leaves a
//! screenshot in target/tmp/e2e/screen.png when `grim` is there.

use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

use serde_json::Value;

const TIMEOUT: Duration = Duration::from_secs(15);

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

/// Kills its process group when dropped, so a failing test leaves nothing.
struct Group(Child);

impl Drop for Group {
    fn drop(&mut self) {
        let _ = Command::new("kill").arg("--").arg(format!("-{}", self.0.id())).status();
        let _ = self.0.wait();
    }
}

fn wait_for<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(start.elapsed() < TIMEOUT, "timed out waiting for {what}");
        sleep(Duration::from_millis(50));
    }
}

struct Wm {
    sock: PathBuf,
}

impl Wm {
    fn try_msg(&self, line: &str) -> Option<Value> {
        let mut stream = UnixStream::connect(&self.sock).ok()?;
        writeln!(stream, "{line}").ok()?;
        let mut reply = String::new();
        BufReader::new(stream).read_line(&mut reply).ok()?;
        serde_json::from_str(&reply).ok()
    }

    fn msg(&self, line: &str) -> Value {
        let reply = self.try_msg(line).unwrap_or_else(|| panic!("no reply to {line}"));
        assert_eq!(reply["ok"], true, "{line}: {reply}");
        reply
    }

    fn tree(&self) -> Value {
        self.msg("tree")["result"].clone()
    }
}

/// The windows of the workspace numbered `number` on the (one) screen.
fn windows(tree: &Value, number: usize) -> Vec<Value> {
    tree["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["number"] == number)
        .map_or_else(Vec::new, |w| w["windows"].as_array().unwrap().clone())
}

fn number(tree: &Value, s: usize) -> u64 {
    seat(tree, s)["workspace_number"].as_u64().unwrap()
}

fn seat(tree: &Value, seat: usize) -> &Value {
    &tree["seats"][seat]
}

fn focus(tree: &Value, s: usize) -> Option<u64> {
    seat(tree, s)["focus"].as_u64()
}

fn view(tree: &Value, s: usize) -> f64 {
    seat(tree, s)["view"].as_f64().unwrap()
}

fn id_at(tree: &Value, workspace: usize, index: usize) -> u64 {
    windows(tree, workspace)[index]["id"].as_u64().unwrap()
}

/// The centre of a window as seat `s` sees it on its screen.
fn centre(tree: &Value, s: usize, window: &Value) -> (i64, i64) {
    let n = |k: &str| window[k].as_i64().unwrap();
    (n("x") - view(tree, s) as i64 + n("width") / 2, n("y") + n("height") / 2)
}

/// Spawn a kitty as seat `s` and wait for its window on that seat's workspace.
fn open_window(wm: &Wm, s: usize) -> u64 {
    launch(wm, s, "kitty --config NONE")
}

/// Run `command` as seat `s` and wait for its window on that seat's workspace.
fn launch(wm: &Wm, s: usize, command: &str) -> u64 {
    let workspace = number(&wm.tree(), s) as usize;
    let before = windows(&wm.tree(), workspace).len();
    wm.msg(&format!("seat {s} spawn {command}"));
    wait_for("a kitty window", || {
        let tree = wm.tree();
        let ws = windows(&tree, workspace);
        (ws.len() > before && ws.iter().all(|w| w["configured"] == true)).then(|| focus(&tree, s).unwrap())
    })
}

/// Events from `wm msg subscribe` until one `want`s it.
fn next_event(events: &mut BufReader<UnixStream>, want: impl Fn(&Value) -> bool) -> Value {
    loop {
        let mut line = String::new();
        events.read_line(&mut line).expect("an event in time");
        assert!(!line.is_empty(), "the subscription ended");
        let event: Value = serde_json::from_str(&line).unwrap();
        if want(&event) {
            return event;
        }
    }
}

/// The windows drawn on screen `index` (in id order).
fn on_screen(tree: &Value, index: usize) -> Vec<u64> {
    tree["screens"][index]["windows"].as_array().unwrap().iter().map(|w| w.as_u64().unwrap()).collect()
}

/// `pid`'s children that exited and weren't reaped.
fn zombies(pid: u32) -> usize {
    let Ok(entries) = fs::read_dir("/proc") else { return 0 };
    entries
        .filter_map(|e| fs::read_to_string(e.ok()?.path().join("stat")).ok())
        .filter(|stat| {
            let fields: Vec<&str> = stat.rsplit_once(')').map_or(vec![], |(_, rest)| rest.split_whitespace().collect());
            fields.first() == Some(&"Z") && fields.get(1) == Some(&pid.to_string().as_str())
        })
        .count()
}

fn start_host(dir: &Path, runtime: &Path) -> (Group, String) {
    let display_file = dir.join("display");
    let config = dir.join("scroll.conf");
    fs::write(
        &config,
        format!(
            "output * resolution 1280x720\ndefault_border none\ngaps inner 0\ngaps outer 0\n\
             exec sh -c 'echo $WAYLAND_DISPLAY > {}'\n",
            display_file.display()
        ),
    )
    .unwrap();
    let host = Command::new("scroll")
        .arg("-c")
        .arg(&config)
        .env("XDG_RUNTIME_DIR", runtime)
        .env("WLR_BACKENDS", "headless")
        .env("WLR_LIBINPUT_NO_DEVICES", "1")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("SWAYSOCK")
        .env_remove("SCROLLSOCK")
        .env_remove("DISPLAY")
        .stdout(Stdio::null())
        .stderr(fs::File::create(dir.join("scroll.log")).unwrap())
        .process_group(0)
        .spawn()
        .expect("start scroll");
    let host = Group(host);
    let display = wait_for("headless scroll", || {
        fs::read_to_string(&display_file).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    });
    (host, display)
}

#[test]
fn nested_in_headless_scroll() {
    if !on_path("scroll") || !on_path("kitty") {
        eprintln!("skipped: needs scroll and kitty");
        return;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("e2e");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    // Its own runtime dir, for the sockets.
    let runtime = dir.join("run");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();

    let (_host, host_display) = start_host(&dir, &runtime);
    // As if wm were started inside another: that one's socket is in
    // $WM_SOCK, and stays its own.
    let outer = dir.join("outer.sock");
    let _outer_listener = UnixListener::bind(&outer).unwrap();
    let log = fs::File::create(dir.join("wm.log")).unwrap();
    let wm_process = Command::new(env!("CARGO_BIN_EXE_wm"))
        .args(["--backend", "winit", "--seats", "2"])
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("WAYLAND_DISPLAY", &host_display)
        .env("WM_SOCK", &outer)
        .env("RUST_LOG", "info,wm::dmabuf=trace")
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .process_group(0)
        .spawn()
        .expect("start wm");
    let mut wm_process = Group(wm_process);
    let wm_pid = wm_process.0.id();
    let sock = wait_for("wm's control socket", || {
        fs::read_dir(&runtime).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with("wm.") && name.ends_with(".sock")
        })
    });
    assert!(UnixStream::connect(&outer).is_ok(), "the outer socket is untouched");
    let wm = Wm { sock };
    wait_for("wm's control socket", || wm.try_msg("tree"));

    // Start: one screen, number 1, recognised by its make, model and serial;
    // two seats, each on its own workspace there; the first is shown.
    let tree = wm.tree();
    let screens = tree["screens"].as_array().unwrap();
    assert_eq!(screens.len(), 1);
    let screen = &screens[0];
    assert_eq!(screen["id"], 1);
    assert_eq!(screen["identity"], "wm nested 0");
    assert_eq!(screen["name"], "winit");
    assert_eq!(screen["seat"], 0);
    assert_eq!(screen["workspaces"].as_array().unwrap().len(), 2);
    assert_eq!(tree["seats"].as_array().unwrap().len(), 2);
    assert_eq!((number(&tree, 0), number(&tree, 1)), (1, 2));
    assert_ne!(seat(&tree, 0)["workspace"], seat(&tree, 1)["workspace"]);
    assert_eq!((seat(&tree, 0)["shown"].as_bool(), seat(&tree, 1)["shown"].as_bool()), (Some(true), Some(false)));
    let out_w = screen["width"].as_i64().unwrap();
    assert!(out_w > 0);

    // Three windows from seat 0: each gets its focus and is scrolled into view.
    let ids: Vec<u64> = (0..3).map(|_| open_window(&wm, 0)).collect();
    let tree = wm.tree();
    assert_eq!(windows(&tree, 1).iter().map(|w| w["id"].as_u64().unwrap()).collect::<Vec<_>>(), ids);
    assert_eq!(focus(&tree, 0), Some(ids[2]));
    let third = &windows(&tree, 1)[2];
    let x = third["x"].as_i64().unwrap() - view(&tree, 0) as i64;
    assert!(x >= 0 && x + third["width"].as_i64().unwrap() <= out_w, "the newest is in view: {third}");
    assert!(view(&tree, 0) > 0.0);

    // Seat 1 launches one, through a launcher that forks it off: it still
    // opens on seat 1's workspace, off screen, and nothing changes for seat 0.
    let theirs = launch(&wm, 1, "setsid -f kitty --config NONE");
    let tree = wm.tree();
    assert_eq!(windows(&tree, 2).len(), 1);
    assert_eq!(id_at(&tree, 2, 0), theirs);
    assert_eq!(focus(&tree, 1), Some(theirs));
    assert_eq!(focus(&tree, 0), Some(ids[2]));
    assert_eq!(tree["screens"][0]["seat"], 0, "still seat 0's workspace on screen");

    // Seat 1 comes over to workspace 1 and scrolls its own view there.
    wm.msg("seat 1 workspace 1");
    wm.msg("seat 1 focus left");
    wm.msg("seat 1 focus left");
    let tree = wm.tree();
    assert_eq!(focus(&tree, 1), Some(ids[0]));
    assert_eq!(view(&tree, 1), 0.0);
    assert!(view(&tree, 0) > 0.0, "seat 0's view stays where it was");
    assert_eq!(seat(&tree, 1)["animating"], false, "seat 1 isn't on screen: no animation");

    // Seat 1 clicks the second window where it sees it: its focus, its view
    // unmoved, seat 0 untouched.
    let (cx, cy) = centre(&tree, 1, &windows(&tree, 1)[1]);
    wm.msg(&format!("seat 1 click {cx} {cy}"));
    let after = wm.tree();
    assert_eq!(focus(&after, 1), Some(ids[1]));
    assert_eq!(view(&after, 1), 0.0);
    assert_eq!(focus(&after, 0), Some(ids[2]));
    assert_eq!(seat(&after, 1)["pointer"]["x"].as_f64().unwrap() as i64, cx);

    // Seat 1's view scrolls under its still pointer; a click there goes to
    // the window now under it, the one it focuses.
    wm.msg("seat 1 focus right");
    assert!(view(&wm.tree(), 1) > 0.0);
    wm.msg("seat 1 click");
    let tree = wm.tree();
    assert_eq!(focus(&tree, 1), Some(ids[2]));
    assert_eq!(seat(&tree, 1)["pointer"]["over"].as_u64(), Some(ids[2]), "the click went where the focus did");

    // Seat 0's animations: off moves at once, on animates and then settles.
    wm.msg("seat 0 animations off");
    wm.msg("seat 0 focus left");
    wm.msg("seat 0 focus left");
    let tree = wm.tree();
    assert_eq!(seat(&tree, 0)["animate"], false);
    assert_eq!(view(&tree, 0), 0.0);
    assert_eq!(seat(&tree, 0)["animating"], false);
    wm.msg("seat 0 animations on");
    wm.msg("seat 0 focus right");
    wm.msg("seat 0 focus right");
    assert_eq!(seat(&wm.tree(), 0)["animating"], true);
    wait_for("the animation to end", || (seat(&wm.tree(), 0)["animating"] == false).then_some(()));

    // Screenshot from the host, for a look.
    if on_path("grim") {
        let _ = Command::new("grim")
            .arg(dir.join("screen.png"))
            .env("XDG_RUNTIME_DIR", &runtime)
            .env("WAYLAND_DISPLAY", &host_display)
            .status();
    }

    // Workspaces: seat 0 sends its window away; both seats on it move on.
    wm.msg("seat 0 focus left");
    wm.msg("seat 1 focus right");
    let tree = wm.tree();
    assert_eq!((focus(&tree, 0), focus(&tree, 1)), (Some(ids[1]), Some(ids[2])));
    wm.msg("seat 0 move-to-workspace 3");
    let tree = wm.tree();
    assert_eq!(windows(&tree, 1).len(), 2);
    assert_eq!(id_at(&tree, 3, 0), ids[1]);
    assert_eq!(focus(&tree, 0), Some(ids[2]), "focus moves on to the neighbour");
    assert_eq!(windows(&tree, 3)[0]["activated"], false, "nobody focuses it there");
    assert_eq!(windows(&tree, 1)[1]["activated"], true);
    assert_eq!(tree["screens"][0]["workspaces"].as_array().unwrap().len(), 3, "made on demand");
    // A subscriber hears the state, then each change as it happens.
    let stream = UnixStream::connect(&wm.sock).unwrap();
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    writeln!(&stream, "subscribe").unwrap();
    let mut events = BufReader::new(stream);
    assert_eq!(next_event(&mut events, |_| true)["ok"], true);
    let seat0 = next_event(&mut events, |e| e["event"] == "seat" && e["id"] == 0);
    assert_eq!((seat0["change"].as_str(), seat0["number"].as_u64()), (Some("new"), Some(1)));
    let is_seat0 = |e: &Value| e["event"] == "seat" && e["id"] == 0;

    wm.msg("seat 0 workspace 3");
    let event = next_event(&mut events, is_seat0);
    assert_eq!((event["change"].as_str(), event["number"].as_u64()), (Some("change"), Some(3)));
    assert_eq!(event["focus"], ids[1]);
    let tree = wm.tree();
    assert_eq!(number(&tree, 0), 3);
    assert_eq!(focus(&tree, 0), Some(ids[1]));
    wm.msg("seat 0 workspace 1");

    // Closing seat 1's window hands seat 1 a neighbour.
    wm.msg("seat 1 close");
    let tree = wait_for("the window to close", || {
        let tree = wm.tree();
        (windows(&tree, 1).len() == 1).then_some(tree)
    });
    assert_eq!(id_at(&tree, 1, 0), ids[0]);
    assert_eq!(focus(&tree, 1), Some(ids[0]));
    next_event(&mut events, |e| e["event"] == "window" && e["change"] == "gone" && e["id"] == ids[2]);
    drop(events);

    // What wm started and has exited is reaped.
    wm.msg("spawn true");
    wait_for("exited children reaped", || (zombies(wm_pid) == 0).then_some(()));

    // Screens: a second (virtual) screen, then unplug the first. Seat 0 was
    // looking at it, so it moves to the one left; seat 1 stays on its
    // workspace, now floating, windows and all.
    let reply = wm.msg("screen-new");
    assert_eq!(reply["result"], 2);
    let tree = wm.tree();
    let second = &tree["screens"][1];
    assert_eq!((second["connected"].as_bool(), second["position"].as_u64()), (Some(true), Some(1)));
    let first_ws = seat(&tree, 0)["workspace"].as_u64().unwrap();
    assert_eq!(seat(&tree, 1)["workspace"].as_u64(), Some(first_ws), "seat 1 is on workspace 1 too");
    assert_eq!(second["seat"], Value::Null, "every seat is seen already");

    // Screen 1's columns past its right edge don't land on screen 2, whose
    // rectangle is next to it: each screen draws only its own.
    let more = [open_window(&wm, 0), open_window(&wm, 0), open_window(&wm, 0)];
    wm.msg("seat 0 animations off");
    for _ in 0..3 {
        wm.msg("seat 0 focus left");
    }
    let tree = wait_for("screen 1 to show the first column", || {
        let tree = wm.tree();
        (view(&tree, 0) == 0.0 && on_screen(&tree, 0).contains(&ids[0])).then_some(tree)
    });
    assert_eq!(windows(&tree, 1).len(), 4);
    let last = &windows(&tree, 1)[3];
    assert!(last["x"].as_i64().unwrap() > out_w, "the last column is past the edge: {last}");
    assert!(!on_screen(&tree, 0).contains(&more[2]));
    assert!(on_screen(&tree, 1).is_empty(), "nothing of screen 1's on screen 2: {:?}", on_screen(&tree, 1));
    wm.msg("seat 0 animations on");

    wm.msg("screen-remove 1");
    let tree = wm.tree();
    assert_eq!(tree["screens"][0]["connected"], false);
    assert_eq!(tree["screens"][1]["position"], 0, "screen 2 is the only one, leftmost");
    let floating = tree["workspaces"].as_array().unwrap().iter().find(|w| w["id"] == first_ws).unwrap().clone();
    assert_eq!(floating["floating"], true);
    assert_eq!(floating["windows"].as_array().unwrap().len(), 4, "its windows are still there");
    assert_eq!(seat(&tree, 0)["workspace"], tree["screens"][1]["active"], "seat 0 moved to screen 2");
    assert_eq!(seat(&tree, 1)["workspace"].as_u64(), Some(first_ws), "seat 1 stays, unseen");
    assert_eq!(seat(&tree, 1)["shown"], false);

    // Seat 0 brings the floating workspace over and goes there.
    wm.msg(&format!("seat 0 take-workspace {first_ws}"));
    let tree = wm.tree();
    assert_eq!(seat(&tree, 0)["workspace"].as_u64(), Some(first_ws));
    assert_eq!(tree["screens"][1]["active"].as_u64(), Some(first_ws), "screen 2 shows it");
    assert!(tree["screens"][0]["workspaces"].as_array().unwrap().iter().all(|w| w.as_u64() != Some(first_ws)));
    assert_eq!(seat(&tree, 0)["shown"], true);
    assert_eq!(focus(&tree, 0), Some(ids[0]), "the window there, focused");

    // Plugged back, screen 1 returns, with the workspaces it kept.
    wm.msg("screen-add 1");
    let tree = wm.tree();
    assert_eq!(tree["screens"][0]["connected"], true);
    assert_eq!(tree["screens"][0]["position"], 1, "back on the right");
    assert!(!tree["screens"][0]["workspaces"].as_array().unwrap().is_empty());
    assert_eq!(tree["screens"][0]["seat"], Value::Null, "no seat is on its workspaces");
    assert_eq!(tree["screens"][1]["seat"], 0, "seat 0 still looks at screen 2");
    assert!(wm.try_msg("screen-remove 9").unwrap()["ok"] == false);

    // Seat 1, unseen on a workspace of its own, gets the next new screen,
    // workspace and all.
    wm.msg("seat 1 workspace 1");
    let own = seat(&wm.tree(), 1)["workspace"].as_u64().unwrap();
    assert_ne!(Some(own), seat(&wm.tree(), 0)["workspace"].as_u64());
    assert_eq!(wm.msg("screen-new")["result"], 3);
    let tree = wm.tree();
    assert_eq!(tree["screens"][2]["seat"], 1);
    assert_eq!(tree["screens"][2]["workspaces"], serde_json::json!([own]));
    assert_eq!((seat(&tree, 0)["shown"].as_bool(), seat(&tree, 1)["shown"].as_bool()), (Some(true), Some(true)));

    // Errors come back as errors.
    let reply = wm.try_msg("seat 7 focus left").unwrap();
    assert_eq!(reply["ok"], false);
    assert_eq!(wm.try_msg("fly").unwrap()["ok"], false);

    // The `wm msg` client.
    let out = Command::new(env!("CARGO_BIN_EXE_wm")).args(["msg", "tree"]).env("WM_SOCK", &wm.sock).output().unwrap();
    assert!(out.status.success());
    let reply: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(reply["result"]["screens"][1]["seat"], 0);
    let out = Command::new(env!("CARGO_BIN_EXE_wm")).args(["msg", "fly"]).env("WM_SOCK", &wm.sock).output().unwrap();
    assert_eq!(out.status.code(), Some(1));

    // Quit.
    wm.msg("quit");
    let exited = wait_for("wm to quit", || wm_process.0.try_wait().unwrap());
    assert!(exited.success(), "wm exited with {exited}");
    assert!(!wm.sock.exists(), "the control socket is removed");
    assert!(outer.exists(), "the outer socket stays");

    // kitty draws with the GPU (Mesa EGL), so its buffers came as dmabufs.
    let log = fs::read_to_string(dir.join("wm.log")).unwrap();
    assert!(log.contains("dmabuf import accepted"), "kitty's dmabufs were imported");
    assert!(!log.contains("dmabuf import rejected"), "no dmabuf was rejected");
}
