//! End to end: wm nested in a headless scroll (nothing shows on screen),
//! with kitty windows, driven and checked through the control socket.
//!
//! Needs `scroll` and `kitty` on PATH (skipped otherwise) and a GPU. Leaves a
//! screenshot in target/tmp/e2e/screen.png (and layers.png, with a bar and
//! fuzzel) when `grim` is there, and dialog.png (seat 0's view of a GTK
//! dialog over its window). The layer-shell step needs `quickshell`,
//! and `fuzzel` for its keyboard part.

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

#[path = "support/locker.rs"]
mod locker;

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
    // A screenshot from the host, for a look.
    let shot = |name: &str| {
        if on_path("grim") {
            let _ = Command::new("grim")
                .arg(dir.join(name))
                .env("XDG_RUNTIME_DIR", &runtime)
                .env("WAYLAND_DISPLAY", &host_display)
                .status();
        }
    };
    // As if wm were started inside another: that one's socket is in
    // $WM_SOCK, and stays its own.
    let outer = dir.join("outer.sock");
    let _outer_listener = UnixListener::bind(&outer).unwrap();
    let log = fs::File::create(dir.join("wm.log")).unwrap();
    let config = dir.join("config.json");
    fs::write(&config, "{}").unwrap();
    let wm_process = Command::new(env!("CARGO_BIN_EXE_wm"))
        // Its own empty config: the default bindings, whatever ~/.config has.
        .args(["--backend", "winit", "--seats", "2", "--config"])
        .arg(&config)
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

    layer_shell(&wm, &dir, ids[2], &shot);

    // Seat 1 launches one, through a launcher that forks it off: it still
    // opens on seat 1's workspace, off screen, and nothing changes for seat 0.
    let theirs = launch(&wm, 1, "setsid -f kitty --config NONE");
    let tree = wm.tree();
    assert_eq!(windows(&tree, 2).len(), 1);
    assert_eq!(id_at(&tree, 2, 0), theirs);
    assert_eq!(focus(&tree, 1), Some(theirs));
    assert_eq!(focus(&tree, 0), Some(ids[2]));
    assert_eq!(tree["screens"][0]["seat"], 0, "still seat 0's workspace on screen");

    // Seat 1's view, though no screen shows it: its own window, at the left
    // of an otherwise empty workspace. Seat 0's is the screen's.
    let png = dir.join("seat1.png");
    let reply = wm.msg(&format!("seat 1 screenshot {}", png.display()));
    assert_eq!((reply["result"]["width"].as_i64(), reply["result"]["height"].as_i64()), (Some(out_w), tree["screens"][0]["height"].as_i64()));
    let (w, pixels) = read_png(&png);
    let at = |x: usize, y: usize| &pixels[(y * w + x) * 4..(y * w + x) * 4 + 4];
    let window = &windows(&tree, 2)[0];
    let (cx, cy) = centre(&tree, 1, window);
    assert_ne!(at(cx as usize, cy as usize), at(w - 4, cy as usize), "the window is drawn: {:?}", at(cx as usize, cy as usize));
    // No title bar of its own (server-side decorations): wm's border, in
    // seat 1's colour, just outside it.
    let edge = at(window["x"].as_u64().unwrap() as usize - 1, cy as usize);
    assert!(edge.iter().zip([235, 92, 92, 255]).all(|(&a, b)| a.abs_diff(b) <= 2), "seat 1's border: {edge:?}");
    let beside = at(w - 4, cy as usize);
    let background = [20, 20, 25, 255];
    assert!(beside.iter().zip(background).all(|(&a, b)| a.abs_diff(b) <= 2), "the background beside it: {beside:?}");
    // Seat 1's pointer, over the gap left of its window (no app there):
    // the theme's arrow, tinted seat 1's red, its tip at the pointer.
    wm.msg("seat 1 pointer 4 300");
    let png = dir.join("cursor.png");
    wm.msg(&format!("seat 1 screenshot {}", png.display()));
    let (w1, pixels) = read_png(&png);
    let red = (300..340)
        .flat_map(|y| (4..40).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let p = &pixels[(y * w1 + x) * 4..(y * w1 + x) * 4 + 3];
            p[0] > 150 && p[1] < 130 && p[2] < 130
        })
        .count();
    assert!(red > 10, "seat 1's cursor is drawn in its colour: {red} red pixels");
    wm.msg(&format!("seat 0 screenshot {}", dir.join("seat0.png").display()));
    assert_eq!(read_png(&dir.join("seat0.png")).0, w);
    assert!(wm.try_msg("seat 1 screenshot relative.png").unwrap()["ok"] == false, "wm wants an absolute path");

    grim_captures(&wm, &dir, &runtime, &tree);

    // Seat 1 comes over to workspace 1 and scrolls its own view there.
    wm.msg("seat 1 workspace 1");
    wm.msg("seat 1 focus left");
    wm.msg("seat 1 focus left");
    let tree = wm.tree();
    assert_eq!(focus(&tree, 1), Some(ids[0]));
    assert_eq!(view(&tree, 1), 0.0);
    assert!(view(&tree, 0) > 0.0, "seat 0's view stays where it was");
    assert_eq!(seat(&tree, 1)["animating"], false, "seat 1 isn't on screen: no animation");

    // Seat 1 clicks the window it sees: its focus, its view unmoved, seat 0
    // untouched.
    let (cx, cy) = centre(&tree, 1, &windows(&tree, 1)[0]);
    wm.msg(&format!("seat 1 click {cx} {cy}"));
    let after = wm.tree();
    assert_eq!(focus(&after, 1), Some(ids[0]));
    assert_eq!(view(&after, 1), 0.0);
    assert_eq!(focus(&after, 0), Some(ids[2]));
    assert_eq!(seat(&after, 1)["pointer"]["x"].as_f64().unwrap() as i64, cx);

    // Seat 1's view scrolls a screen over, under its still pointer; a click
    // there goes to the window now under it, the one it focuses.
    wm.msg("seat 1 focus right");
    assert!(view(&wm.tree(), 1) > 0.0);
    wm.msg("seat 1 click");
    let tree = wm.tree();
    assert_eq!(focus(&tree, 1), Some(ids[1]));
    assert_eq!(seat(&tree, 1)["pointer"]["over"].as_u64(), Some(ids[1]), "the click went where the focus did");

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

    shot("screen.png");

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

    // Seat 0 jumps to its workspace's first and last windows, then takes
    // the last one to workspace 4 and goes with it, still focused on it.
    let own_windows = |tree: &Value| -> Vec<u64> {
        let ws = seat(tree, 0)["workspace"].clone();
        let ws = tree["workspaces"].as_array().unwrap().iter().find(|w| w["id"] == ws).unwrap();
        ws["windows"].as_array().unwrap().iter().map(|w| w["id"].as_u64().unwrap()).collect()
    };
    let list = own_windows(&wm.tree());
    assert!(list.len() >= 2, "seat 0 has windows: {list:?}");
    wm.msg("seat 0 focus first");
    assert_eq!(focus(&wm.tree(), 0), Some(list[0]));
    wm.msg("seat 0 focus last");
    let last = *list.last().unwrap();
    assert_eq!(focus(&wm.tree(), 0), Some(last));
    wm.msg("seat 0 move-to-workspace 4 follow");
    let tree = wm.tree();
    assert_eq!(seat(&tree, 0)["number"], 4);
    assert_eq!(focus(&tree, 0), Some(last));
    assert_eq!(own_windows(&tree), vec![last]);

    dialog_floats(&wm, &dir);
    menu_grabs(&wm, &dir);
    x11_apps(&wm, &dir);
    session_lock(&wm, &dir, &runtime);

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

/// Run `command` as seat `s`, its pid written to `dir/<name>.pid`.
fn spawn_tracked(wm: &Wm, s: usize, dir: &Path, name: &str, command: &str) -> PathBuf {
    let pid = dir.join(format!("{name}.pid"));
    let _ = fs::remove_file(&pid);
    wm.msg(&format!("seat {s} spawn echo $$ > {}; exec {command}", pid.display()));
    pid
}

fn kill(pid: &Path) {
    let pid = wait_for("a pid file", || fs::read_to_string(pid).ok().filter(|p| !p.trim().is_empty()));
    let _ = Command::new("kill").arg(pid.trim()).status();
}

/// A Quickshell bar keeps its room from the windows, and fuzzel takes the
/// keyboard of the seat that launched it, giving it back when it goes.
fn layer_shell(wm: &Wm, dir: &Path, focused: u64, shot: &dyn Fn(&str)) {
    if !on_path("quickshell") {
        eprintln!("layer shell: skipped, needs quickshell");
        return;
    }
    let qml = dir.join("bar.qml");
    fs::write(
        &qml,
        r#"import Quickshell
import Quickshell.Wayland
import QtQuick
ShellRoot {
    PanelWindow {
        anchors { top: true; left: true; right: true }
        implicitHeight: 30
        color: "steelblue"
        WlrLayershell.namespace: "e2e-bar"
    }
}
"#,
    )
    .unwrap();
    let full = wm.tree()["screens"][0]["area"].clone();
    let bar = spawn_tracked(wm, 0, dir, "bar", &format!("quickshell -p {}", qml.display()));
    // Its room is kept once it asks; its size shows with its first frame.
    let tree = wait_for("the bar, drawn, and its room", || {
        let tree = wm.tree();
        let screen = &tree["screens"][0];
        (screen["area"]["y"] == 30 && screen["layers"][0]["height"] == 30).then_some(tree)
    });
    let screen = &tree["screens"][0];
    assert_eq!(screen["layers"][0]["namespace"], "e2e-bar");
    assert_eq!(screen["layers"][0]["layer"], "top");
    assert_eq!(screen["layers"][0]["height"], 30);
    assert_eq!(screen["area"]["height"], full["height"].as_i64().unwrap() - 30);
    for window in windows(&tree, 1) {
        assert!(window["y"].as_i64().unwrap() > 30, "below the bar: {window}");
    }
    // The bar wants no keyboard: a click on it leaves the focus be.
    wm.msg("seat 0 click 5 5");
    let tree = wm.tree();
    assert_eq!(focus(&tree, 0), Some(focused));
    assert_eq!(seat(&tree, 0)["layer_focus"], Value::Null);

    if on_path("fuzzel") {
        // dmenu mode with nothing to pick: a stray click can't start an app.
        let launcher = spawn_tracked(wm, 0, dir, "fuzzel", "fuzzel --dmenu < /dev/null");
        wait_for("fuzzel to take seat 0's keyboard", || {
            (seat(&wm.tree(), 0)["layer_focus"] == "launcher").then_some(())
        });
        assert_eq!(seat(&wm.tree(), 1)["layer_focus"], Value::Null, "not seat 1's");
        // A click on a window (near its top, clear of fuzzel in the middle)
        // focuses it but leaves fuzzel the keyboard.
        let tree = wm.tree();
        let (cx, _) = centre(&tree, 0, &windows(&tree, 1)[1]);
        let top = windows(&tree, 1)[1]["y"].as_i64().unwrap() + 20;
        wm.msg(&format!("seat 0 click {cx} {top}"));
        let tree = wm.tree();
        assert_eq!(seat(&tree, 0)["pointer"]["over"], windows(&tree, 1)[1]["id"], "the click was on the window");
        assert_eq!(seat(&tree, 0)["layer_focus"], "launcher");
        wm.msg("seat 0 focus right");
        shot("layers.png");
        kill(&launcher);
        wait_for("the keyboard back on the window", || {
            (seat(&wm.tree(), 0)["layer_focus"] == Value::Null).then_some(())
        });
        assert_eq!(focus(&wm.tree(), 0), Some(focused));
    } else {
        eprintln!("layer shell: fuzzel skipped, not on PATH");
    }

    // Full screen: the focused window covers the screen, bar and all; the
    // bar is neither drawn nor clicked; back, it is below the bar again.
    wm.msg("seat 0 fullscreen");
    let tree = wm.tree();
    let window = windows(&tree, 1).into_iter().find(|w| w["id"] == focused).unwrap();
    let screen = &tree["screens"][0];
    assert_eq!(window["fullscreen"], true);
    assert_eq!((&window["width"], &window["height"]), (&screen["width"], &screen["height"]));
    assert_eq!(window["y"], 0);
    assert_eq!(window["x"].as_f64().unwrap() - view(&tree, 0), 0.0, "seat 0's view has it at the edge");
    let png = dir.join("fullscreen.png");
    wait_for("the bar hidden under the window", || {
        wm.msg(&format!("seat 0 screenshot {}", png.display()));
        let (w, pixels) = read_png(&png);
        let at = |x: usize, y: usize| pixels[(y * w + x) * 4..(y * w + x) * 4 + 3].to_vec();
        // Steel blue is the bar; the window's own is anything else.
        (at(w / 2, 10) != [70, 130, 180]).then_some(())
    });
    wm.msg("seat 0 click 300 10");
    assert_eq!(seat(&wm.tree(), 0)["pointer"]["over"], focused, "the click lands on the window, not the bar");
    wm.msg("seat 0 fullscreen off");
    let tree = wm.tree();
    let window = windows(&tree, 1).into_iter().find(|w| w["id"] == focused).unwrap();
    assert_eq!(window["fullscreen"], false);
    assert!(window["y"].as_i64().unwrap() > 30, "below the bar again: {window}");

    kill(&bar);
    wait_for("the bar's room back", || (wm.tree()["screens"][0]["area"] == full).then_some(()));
}

/// Other programs capture through ext-image-copy-capture: grim takes the
/// screen as seat 0's screenshot shows it, and seat 1's window, though no
/// screen shows it, by its id in the tree.
fn grim_captures(wm: &Wm, dir: &Path, runtime: &Path, tree: &Value) {
    if !on_path("grim") {
        eprintln!("grim captures skipped: needs grim");
        return;
    }
    let display = wm.sock.file_name().unwrap().to_string_lossy();
    let display = display.strip_prefix("wm.").unwrap().strip_suffix(".sock").unwrap().to_string();
    let grim = |args: &[&str], name: &str| -> (usize, usize, Vec<u8>) {
        let path = dir.join(name);
        let status = Command::new("grim")
            .args(["-t", "ppm"])
            .args(args)
            .arg(&path)
            .env("XDG_RUNTIME_DIR", runtime)
            .env("WAYLAND_DISPLAY", &display)
            .status()
            .unwrap();
        assert!(status.success(), "grim {args:?}");
        read_ppm(&path)
    };
    let pixel = |w: usize, rgb: &[u8], x: usize, y: usize| rgb[(y * w + x) * 3..(y * w + x) * 3 + 3].to_vec();
    let near = |a: &[u8], b: &[u8]| a.iter().zip(b).all(|(&a, &b)| a.abs_diff(b) <= 2);

    // The screen: seat 0's view, as its screenshot has it.
    let screen = &tree["screens"][0];
    let (w, h, rgb) = grim(&["-o", screen["name"].as_str().unwrap()], "grim-screen.ppm");
    assert_eq!((w as i64, h as i64), (screen["width"].as_i64().unwrap(), screen["height"].as_i64().unwrap()));
    // All but seat 0's pointer (grim leaves it out) and a blinking cursor.
    let (_, rgba) = read_png(&dir.join("seat0.png"));
    let differ = rgb.as_chunks::<3>().0.iter().zip(rgba.as_chunks::<4>().0).filter(|(a, b)| !near(&a[..], &b[..3])).count();
    assert!(differ < w * h / 100, "grim and wm's screenshot agree: {differ} pixels differ");

    // Seat 1's window, off screen, alone at its own size.
    let theirs = &windows(tree, 2)[0];
    let id = theirs["id"].as_u64().unwrap().to_string();
    let (w, h, rgb) = grim(&["-T", &id], "grim-window.ppm");
    assert_eq!((w as i64, h as i64), (theirs["width"].as_i64().unwrap(), theirs["height"].as_i64().unwrap()));
    assert!(!near(&pixel(w, &rgb, w / 2, h / 2), &[20, 20, 25]), "the window, not the background");
}

/// A binary PPM (grim -t ppm): width, height, RGB rows.
fn read_ppm(path: &Path) -> (usize, usize, Vec<u8>) {
    let data = fs::read(path).unwrap();
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while data[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while !data[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&data[start..i]).to_string());
    }
    assert_eq!(fields[0], "P6");
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    (w, h, data[i + 1..].to_vec())
}

/// A PNG's width and RGBA pixels.
fn read_png(path: &Path) -> (usize, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    (info.width as usize, pixels)
}

/// A GTK window opens a dialog for itself: the dialog floats over it, centred,
/// at its own size, focused; the window has a full-screen column.
fn dialog_floats(wm: &Wm, dir: &Path) {
    let gtk = Command::new("python3")
        .args(["-c", "import gi; gi.require_version('Gtk', '4.0'); from gi.repository import Gtk"])
        .status()
        .is_ok_and(|s| s.success());
    if !gtk {
        eprintln!("dialogs: skipped, needs python3 with GTK 4");
        return;
    }
    let script = dir.join("dialog.py");
    fs::write(
        &script,
        r#"import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib
Gtk.init()
parent = Gtk.Window(title="e2e-parent")
parent.present()
def dialog():
    Gtk.Window(title="e2e-dialog", transient_for=parent, default_width=320, default_height=180).present()
    return False
GLib.timeout_add(300, dialog)
GLib.MainLoop().run()
"#,
    )
    .unwrap();
    let pid = spawn_tracked(wm, 0, dir, "dialog", &format!("env GDK_BACKEND=wayland python3 {}", script.display()));
    let titled = |tree: &Value, title: &str| -> Option<Value> {
        tree["workspaces"].as_array()?.iter().flat_map(|ws| ws["windows"].as_array().unwrap().iter()).find(|w| w["title"] == title).cloned()
    };
    let (parent, dialog) = wait_for("the window and its dialog", || {
        let tree = wm.tree();
        let (parent, dialog) = (titled(&tree, "e2e-parent")?, titled(&tree, "e2e-dialog")?);
        (dialog["configured"] == true && dialog["width"].as_i64()? > 0).then_some((parent, dialog))
    });
    let n = |w: &Value, k: &str| w[k].as_i64().unwrap();
    assert_eq!((parent["floating"].as_bool(), dialog["floating"].as_bool()), (Some(false), Some(true)));
    assert_eq!(parent["workspace"], dialog["workspace"]);
    assert!(n(&dialog, "width") < n(&parent, "width"), "its own size, not a column's: {dialog}");
    for (x, w) in [("x", "width"), ("y", "height")] {
        let (parent_mid, dialog_mid) = (n(&parent, x) + n(&parent, w) / 2, n(&dialog, x) + n(&dialog, w) / 2);
        assert!((parent_mid - dialog_mid).abs() <= 1, "centred over its parent along {x}: {parent} {dialog}");
    }
    assert_eq!(focus(&wm.tree(), 0), dialog["id"].as_u64(), "the dialog has the focus");
    wm.msg(&format!("seat 0 screenshot {}", dir.join("dialog.png").display()));

    // Focus follows the mouse, once the config says so (and `reload`):
    // onto the window beside the dialog, then back onto the dialog.
    let config = dir.join("config.json");
    fs::write(&config, r#"{"input": {"focus_follows_mouse": true}}"#).unwrap();
    wm.msg("seat 0 reload");
    let tree = wm.tree();
    let (px, py) = centre(&tree, 0, &parent);
    wm.msg(&format!("seat 0 pointer {} {}", px - n(&parent, "width") / 2 + 30, py));
    assert_eq!(focus(&wm.tree(), 0), parent["id"].as_u64(), "the window under the pointer");
    let (dx, dy) = centre(&tree, 0, &dialog);
    wm.msg(&format!("seat 0 pointer {dx} {dy}"));
    assert_eq!(focus(&wm.tree(), 0), dialog["id"].as_u64(), "back on the dialog");
    fs::write(&config, "{}").unwrap();
    wm.msg("seat 0 reload");
    kill(&pid);
    wait_for("the window and dialog to go", || {
        let tree = wm.tree();
        (titled(&tree, "e2e-parent").is_none() && titled(&tree, "e2e-dialog").is_none()).then_some(())
    });
}

/// A GTK window's menu (an autohide popover, an xdg_popup grab) holds the
/// pointer and keyboard of seat 0, which opened it, and no one else's; a
/// click of seat 0's outside it closes it.
fn menu_grabs(wm: &Wm, dir: &Path) {
    let gtk = Command::new("python3")
        .args(["-c", "import gi; gi.require_version('Gtk', '4.0'); from gi.repository import Gtk"])
        .status()
        .is_ok_and(|s| s.success());
    if !gtk {
        eprintln!("menus: skipped, needs python3 with GTK 4");
        return;
    }
    let closed = dir.join("menu-closed");
    let script = dir.join("menu.py");
    fs::write(
        &script,
        format!(
            r#"import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib
Gtk.init()
window = Gtk.Window(title="e2e-menu")
box = Gtk.Box(valign=Gtk.Align.START, halign=Gtk.Align.START)
button = Gtk.Button(label="menu")
box.append(button)
window.set_child(box)
popover = Gtk.Popover(autohide=True, child=Gtk.Label(label="an item"))
popover.set_parent(button)
popover.connect("closed", lambda _: open({closed:?}, "w").write("closed"))
window.present()
GLib.timeout_add(500, lambda: popover.popup() and False)
GLib.MainLoop().run()
"#,
            closed = closed.display().to_string()
        ),
    )
    .unwrap();
    let pid = spawn_tracked(wm, 0, dir, "menu", &format!("env GDK_BACKEND=wayland python3 {}", script.display()));
    wait_for("the menu to grab seat 0", || (seat(&wm.tree(), 0)["grab"] == true).then_some(()));
    let tree = wm.tree();
    assert_eq!(seat(&tree, 1)["grab"], false, "seat 1 is free");
    wm.msg(&format!("seat 0 screenshot {}", dir.join("menu.png").display()));
    assert!(!closed.exists());

    // Seat 0 clicks low in the window, far from the menu by the button.
    let window = tree["workspaces"].as_array().unwrap().iter()
        .flat_map(|ws| ws["windows"].as_array().unwrap().iter())
        .find(|w| w["title"] == "e2e-menu").unwrap().clone();
    let (x, y) = centre(&tree, 0, &window);
    wm.msg(&format!("seat 0 click {x} {}", y + window["height"].as_i64().unwrap() / 3));
    wait_for("the menu closed", || closed.exists().then_some(()));
    // Free again: its next frame lets go of the grab, the keyboard is back
    // on the window.
    wait_for("seat 0 free again", || (seat(&wm.tree(), 0)["grab"] == false).then_some(()));
    assert_eq!(focus(&wm.tree(), 0), window["id"].as_u64());
    kill(&pid);
}

/// An X11 app (GTK on its X11 backend) through xwayland-satellite: its
/// window is an ordinary one in the layout, and its transient dialog floats
/// over it.
fn x11_apps(wm: &Wm, dir: &Path) {
    let tree = wm.tree();
    let Some(display) = tree["x_display"].as_str() else {
        // There but not serving: it died (wm.log has why).
        assert!(!on_path("xwayland-satellite"), "xwayland-satellite is on PATH but serves no display");
        eprintln!("X11 apps: skipped, needs xwayland-satellite");
        return;
    };
    assert!(display.starts_with(':'), "{display}");
    let script = dir.join("x11.py");
    fs::write(
        &script,
        r#"import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib
Gtk.init()
parent = Gtk.Window(title="e2e-x11")
parent.present()
def dialog():
    Gtk.Window(title="e2e-x11-dialog", transient_for=parent, default_width=320, default_height=180).present()
    return False
GLib.timeout_add(500, dialog)
GLib.MainLoop().run()
"#,
    )
    .unwrap();
    let pid = spawn_tracked(wm, 0, dir, "x11", &format!("env GDK_BACKEND=x11 python3 {}", script.display()));
    let titled = |tree: &Value, title: &str| -> Option<Value> {
        tree["workspaces"].as_array()?.iter().flat_map(|ws| ws["windows"].as_array().unwrap().iter()).find(|w| w["title"] == title).cloned()
    };
    let (parent, dialog) = wait_for("the X11 window and its dialog", || {
        let tree = wm.tree();
        let (parent, dialog) = (titled(&tree, "e2e-x11")?, titled(&tree, "e2e-x11-dialog")?);
        (dialog["width"].as_i64()? > 0).then_some((parent, dialog))
    });
    assert_eq!((parent["floating"].as_bool(), dialog["floating"].as_bool()), (Some(false), Some(true)), "{parent} {dialog}");
    assert_eq!(parent["workspace"], dialog["workspace"]);
    assert_eq!(focus(&wm.tree(), 0), dialog["id"].as_u64(), "the dialog has seat 0's focus");
    wm.msg(&format!("seat 0 screenshot {}", dir.join("x11.png").display()));
    kill(&pid);
    wait_for("the X11 windows to go", || {
        let tree = wm.tree();
        (titled(&tree, "e2e-x11").is_none() && titled(&tree, "e2e-x11-dialog").is_none()).then_some(())
    });
}

/// A locker locks every seat: both keyboards go to it, commands and
/// screenshots are refused, the screen shows only it. A second locker is
/// refused; the first dying leaves the session locked until another takes
/// over and unlocks.
fn session_lock(wm: &Wm, dir: &Path, runtime: &Path) {
    let display = wm.sock.file_name().unwrap().to_string_lossy();
    let display = display.strip_prefix("wm.").unwrap().strip_suffix(".sock").unwrap().to_string();
    let refused = |line: &str| {
        let reply = wm.try_msg(line).unwrap();
        reply["ok"] == false && reply["error"].as_str().is_some_and(|e| e.contains("locked"))
    };
    let focused = focus(&wm.tree(), 0);

    let mut first = locker::Locker::lock(runtime, &display, [200, 30, 30], dir);
    first.wait("the lock", |s| s.locked);
    assert_eq!(wm.tree()["locked"], true);
    first.wait("both seats' keyboards on the lock surface", |s| s.drawn >= 1 && s.keyboards.len() == 2);
    assert!(refused("seat 1 focus left"), "commands are refused while locked");
    assert!(refused("seat 0 spawn true"));
    assert!(refused(&format!("screenshot {}", dir.join("locked.png").display())), "screenshots are refused");
    assert!(!dir.join("locked.png").exists());
    if on_path("grim") {
        let path = dir.join("grim-locked.ppm");
        let screen = wm.tree()["screens"][0]["name"].as_str().unwrap().to_string();
        let ok = Command::new("grim")
            .args(["-t", "ppm", "-o", &screen])
            .arg(&path)
            .env("XDG_RUNTIME_DIR", runtime)
            .env("WAYLAND_DISPLAY", &display)
            .status()
            .unwrap()
            .success();
        assert!(ok, "grim while locked");
        let (w, h, rgb) = read_ppm(&path);
        let red = rgb.as_chunks::<3>().0.iter().filter(|p| p[0] > 150 && p[1] < 80 && p[2] < 80).count();
        assert!(red > w * h * 9 / 10, "the screen shows only the locker: {red} of {} pixels red", w * h);
    }

    let mut second = locker::Locker::lock(runtime, &display, [30, 200, 30], dir);
    second.wait("the second locker's refusal", |s| s.finished);
    assert!(!second.seen().locked);
    drop(second);

    first.crash();
    sleep(Duration::from_millis(300));
    assert_eq!(wm.tree()["locked"], true, "a crashed locker leaves the session locked");
    assert!(refused("seat 0 focus left"));
    let mut third = locker::Locker::lock(runtime, &display, [30, 30, 200], dir);
    third.wait("a new locker taking over", |s| s.locked && s.keyboards.len() == 2);
    third.unlock();
    wait_for("the unlock", || (wm.tree()["locked"] == false).then_some(()));
    assert_eq!(focus(&wm.tree(), 0), focused, "focus is as it was");
    assert_eq!(wm.try_msg("seat 0 focus left").unwrap()["ok"], true);
}
