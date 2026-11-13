//! Unit tests, a module per subject: `cargo test --test unit layout::` runs one.

mod layout {
    use std::{
        cell::Cell,
        rc::Rc,
        time::{Duration, Instant},
    };

    use smithay::utils::{Logical, Rectangle, Size};
    use wm::layout::{DEFAULT_WIDTH, DURATION, Direction, GAP, Tile, Workspace};

    /// A window that remembers the size it was asked to take.
    #[derive(Debug, Clone)]
    struct Fake {
        id: u32,
        size: Rc<Cell<Option<(i32, i32)>>>,
    }

    impl PartialEq for Fake {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    impl Tile for Fake {
        fn configure(&self, size: Size<i32, Logical>) {
            self.size.set(Some((size.w, size.h)));
        }
    }

    fn fake(id: u32) -> Fake {
        Fake { id, size: Rc::default() }
    }

    const OUT: (i32, i32) = (1000, 600);
    const ME: usize = 0;
    const OTHER: usize = 1;

    fn out() -> Size<i32, Logical> {
        OUT.into()
    }

    fn column_width() -> i32 {
        (OUT.0 as f64 * DEFAULT_WIDTH) as i32 - GAP - GAP / 2
    }

    /// Workspace with windows 1..=n, added in order, animated.
    fn workspace(n: u32, now: Instant) -> (Workspace<Fake>, Vec<Fake>) {
        let mut ws = Workspace::new(out());
        let windows: Vec<Fake> = (1..=n).map(fake).collect();
        for w in &windows {
            ws.add(w.clone(), None, now, true);
        }
        (ws, windows)
    }

    fn xs(ws: &Workspace<Fake>) -> Vec<(u32, i32)> {
        ws.targets().map(|(w, r)| (w.id, r.loc.x)).collect()
    }

    fn screen_xs(ws: &Workspace<Fake>, seat: usize, now: Instant) -> Vec<i32> {
        ws.positions(seat, now).map(|(_, p)| p.x).collect()
    }

    /// Third window's right edge, gap included, minus the screen: the view that
    /// just shows it.
    fn view_showing_third() -> f64 {
        let w = column_width();
        (3 * GAP + 2 * w + w + GAP - OUT.0) as f64
    }

    #[test]
    fn columns_fill_from_the_left_with_gaps() {
        let now = Instant::now();
        let (ws, windows) = workspace(3, now);
        let w = column_width();
        assert_eq!(xs(&ws), vec![(1, GAP), (2, 2 * GAP + w), (3, 3 * GAP + 2 * w)]);
        // Two half-width columns and their gaps fill the screen exactly.
        assert_eq!(3 * GAP + 2 * w, OUT.0);
        for window in &windows {
            assert_eq!(window.size.get(), Some((w, OUT.1 - 2 * GAP)));
        }
        let rect = ws.targets().next().unwrap().1;
        assert_eq!((rect.loc.y, rect.size.h), (GAP, OUT.1 - 2 * GAP));
    }

    #[test]
    fn adds_after_a_given_window() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(2, now);
        ws.add(fake(9), Some(&windows[0]), now, true);
        let order: Vec<u32> = ws.windows().map(|w| w.id).collect();
        assert_eq!(order, vec![1, 9, 2]);
    }

    #[test]
    fn reveal_scrolls_only_as_far_as_needed() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(3, now);
        assert_eq!(ws.view_target(ME), 0.0);
        ws.reveal(ME, &windows[1], now, true);
        assert_eq!(ws.view_target(ME), 0.0, "visible already: nothing moves");
        ws.reveal(ME, &windows[2], now, true);
        assert_eq!(ws.view_target(ME), view_showing_third());
        ws.reveal(ME, &windows[0], now, true);
        assert_eq!(ws.view_target(ME), 0.0, "back to the first's left edge, gap included");
    }

    #[test]
    fn each_seat_has_its_own_view() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(3, now);
        ws.reveal(OTHER, &windows[2], now, false);
        assert_eq!(ws.view_target(OTHER), view_showing_third());
        assert_eq!(ws.view_target(ME), 0.0, "the other seat scrolling doesn't scroll mine");
        let later = now + DURATION * 2;
        assert_eq!(screen_xs(&ws, ME, later)[0], GAP);
        assert!(screen_xs(&ws, OTHER, later)[0] < 0, "the other seat sees the first one scrolled off");
    }

    #[test]
    fn removing_hands_focus_to_a_neighbour_and_closes_the_gap() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(3, now);
        assert_eq!(ws.remove(&windows[1], now, true).map(|w| w.id), Some(3));
        assert_eq!(xs(&ws), vec![(1, GAP), (3, 2 * GAP + column_width())]);
        assert_eq!(ws.remove(&windows[2], now, true).map(|w| w.id), Some(1));
        assert_eq!(ws.remove(&windows[0], now, true), None);
        assert!(ws.is_empty());
    }

    #[test]
    fn removing_pulls_every_view_back() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(3, now);
        ws.reveal(ME, &windows[2], now, true);
        ws.reveal(OTHER, &windows[2], now, true);
        ws.remove(&windows[2], now, true);
        assert_eq!(ws.view_target(ME), 0.0, "two columns fit: no need to scroll");
        assert_eq!(ws.view_target(OTHER), 0.0);
    }

    #[test]
    fn neighbours_and_moves() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(3, now);
        assert_eq!(ws.neighbour(&windows[0], Direction::Left), None);
        assert_eq!(ws.neighbour(&windows[0], Direction::Right).map(|w| w.id), Some(2));
        assert_eq!(ws.neighbour(&windows[2], Direction::Right), None);
        ws.move_column(&windows[0], Direction::Right, now, true);
        assert_eq!(ws.windows().map(|w| w.id).collect::<Vec<_>>(), vec![2, 1, 3]);
        ws.move_column(&windows[2], Direction::Right, now, true);
        assert_eq!(ws.windows().map(|w| w.id).collect::<Vec<_>>(), vec![2, 1, 3], "already last");
    }

    #[test]
    fn animates_then_settles() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(3, now);
        ws.reveal(ME, &windows[2], now, true);
        assert!(ws.is_animating(ME, now));
        assert_eq!(screen_xs(&ws, ME, now)[0], GAP, "the view starts where it was");
        let later = now + DURATION + Duration::from_millis(1);
        assert!(!ws.is_animating(ME, later));
        let target: Vec<i32> = ws.targets().map(|(_, r)| r.loc.x - view_showing_third() as i32).collect();
        assert_eq!(screen_xs(&ws, ME, later), target);
    }

    #[test]
    fn a_seat_without_animations_jumps_while_another_animates() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(3, now);
        let settled = now + DURATION * 2;
        ws.reveal(ME, &windows[2], settled, true);
        ws.reveal(OTHER, &windows[2], settled, false);
        assert!(ws.is_animating(ME, settled));
        assert!(!ws.is_animating(OTHER, settled), "the other seat is there already");
        assert_eq!(ws.view(OTHER, settled), view_showing_third());
    }

    #[test]
    fn unanimated_changes_jump() {
        let now = Instant::now();
        let mut ws = Workspace::new(out());
        let windows: Vec<Fake> = (1..=3).map(fake).collect();
        for w in &windows {
            ws.add(w.clone(), None, now, false);
        }
        ws.move_column(&windows[0], Direction::Right, now, false);
        assert!(!ws.is_animating(ME, now));
        let target: Vec<i32> = ws.targets().map(|(_, r)| r.loc.x).collect();
        assert_eq!(screen_xs(&ws, ME, now), target);
    }

    #[test]
    fn finish_stops_everything_where_it_was_headed() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(3, now);
        ws.reveal(ME, &windows[2], now, true);
        ws.move_column(&windows[0], Direction::Right, now, true);
        ws.finish();
        assert!(!ws.is_animating(ME, now));
        assert_eq!(ws.view(ME, now), view_showing_third());
    }

    #[test]
    fn new_window_appears_in_place_while_others_slide() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(2, now);
        let settled = now + DURATION * 2;
        ws.add(fake(9), Some(&windows[0]), settled, true);
        let pos: Vec<(u32, i32)> = ws.positions(ME, settled).map(|(w, p)| (w.id, p.x)).collect();
        let w = column_width();
        assert_eq!(pos[1], (9, 2 * GAP + w), "the new one starts where it belongs");
        assert_eq!(pos[2], (2, 2 * GAP + w), "the one it displaced starts where it was");
        assert!(ws.is_animating(ME, settled));
    }

    #[test]
    fn resizing_relays_columns() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(3, now);
        ws.reveal(ME, &windows[2], now, false);
        let wide: Size<i32, Logical> = (3000, 600).into();
        ws.resize(wide, now, false);
        assert_eq!(ws.size(), wide);
        let w = (3000.0 * DEFAULT_WIDTH) as i32 - GAP - GAP / 2;
        assert_eq!(windows[0].size.get(), Some((w, 600 - 2 * GAP)));
        // Three half-width columns still don't fit, and the view is in range: kept.
        assert_eq!(ws.view_target(ME), view_showing_third());
        // Narrower than before, the last column ends further left: pulled back to it.
        ws.resize((600, 600).into(), now, false);
        let w = (600.0 * DEFAULT_WIDTH) as i32 - GAP - GAP / 2;
        let end = (GAP + 3 * (w + GAP) - 600) as f64;
        assert!(end < view_showing_third());
        assert_eq!(ws.view_target(ME), end);
    }

    #[test]
    fn a_bar_takes_room_from_the_area() {
        let now = Instant::now();
        let (mut ws, windows) = workspace(2, now);
        // A 30 px bar along the top: the columns start under it and are
        // that much shorter; along the screen nothing changes.
        ws.set_area(Rectangle::new((0, 30).into(), (OUT.0, OUT.1 - 30).into()), now, false);
        assert_eq!(windows[0].size.get(), Some((column_width(), OUT.1 - 30 - 2 * GAP)));
        let pos: Vec<_> = ws.positions(ME, now).map(|(w, p)| (w.id, (p.x, p.y))).collect();
        assert_eq!(pos, vec![(1, (GAP, 30 + GAP)), (2, (2 * GAP + column_width(), 30 + GAP))]);
        assert_eq!(ws.targets().next().unwrap().1.loc.y, 30 + GAP);
        // One along the left edge shifts them right.
        ws.set_area(Rectangle::new((40, 0).into(), (OUT.0 - 40, OUT.1).into()), now, false);
        assert_eq!(ws.positions(ME, now).next().unwrap().1, (40 + GAP, GAP).into());
    }
}

mod animation {
    use std::time::{Duration, Instant};

    use wm::layout::{Anim, DURATION};

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn starts_still() {
        let a = Anim::new(5.0);
        let now = Instant::now();
        assert_eq!(a.value(now), 5.0);
        assert!(a.is_done(now));
    }

    #[test]
    fn moves_to_its_target_and_stops() {
        let t0 = Instant::now();
        let mut a = Anim::new(0.0);
        a.set(100.0, t0, true);
        assert_eq!(a.value(t0), 0.0);
        let half = a.value(at(t0, DURATION.as_millis() as u64 / 2));
        assert!(half > 50.0 && half < 100.0, "eases out: past halfway at half time, got {half}");
        assert!(!a.is_done(at(t0, 10)));
        let end = t0 + DURATION;
        assert_eq!(a.value(end), 100.0);
        assert!(a.is_done(end));
        assert_eq!(a.value(at(t0, 10_000)), 100.0);
    }

    #[test]
    fn never_goes_backwards_when_retargeted() {
        // A second change mid-way continues from where it is, not from the
        // first start: what used to make scroll's animations jump.
        let t0 = Instant::now();
        let mut a = Anim::new(0.0);
        a.set(100.0, t0, true);
        let t1 = at(t0, 100);
        let before = a.value(t1);
        a.set(200.0, t1, true);
        assert_eq!(a.value(t1), before);
        let mut last = before;
        for ms in (100..=400).step_by(10) {
            let v = a.value(at(t0, ms));
            assert!(v >= last, "went back from {last} to {v} at {ms}ms");
            last = v;
        }
        assert_eq!(last, 200.0);
    }

    #[test]
    fn same_target_doesnt_restart() {
        let t0 = Instant::now();
        let mut a = Anim::new(0.0);
        a.set(100.0, t0, true);
        let t1 = at(t0, 200);
        let v = a.value(t1);
        a.set(100.0, t1, true);
        assert_eq!(a.value(t1), v);
        assert!(a.is_done(t0 + DURATION));
    }

    #[test]
    fn unanimated_jumps() {
        let t0 = Instant::now();
        let mut a = Anim::new(0.0);
        a.set(100.0, t0, false);
        assert_eq!(a.value(t0), 100.0);
        assert!(a.is_done(t0));
    }

    #[test]
    fn finish_stops_at_the_target() {
        let t0 = Instant::now();
        let mut a = Anim::new(0.0);
        a.set(100.0, t0, true);
        a.finish();
        assert_eq!(a.value(t0), 100.0);
        assert!(a.is_done(t0));
    }
}

mod screens {
    use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
    use wm::state::{ConnectorId, Screen, ScreenId, WorkspaceId, identity, identity_of, returning};

    fn output(connector: &str, make: &str, model: &str, serial: &str) -> Output {
        Output::new(
            connector.into(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: make.into(),
                model: model.into(),
                serial_number: serial.into(),
            },
        )
    }

    fn screen(id: u32, output: Output) -> Screen {
        let identity = identity_of(&output, []);
        Screen::new(ScreenId(id), identity, output, vec![WorkspaceId(1)])
    }

    #[test]
    fn identity_is_make_model_serial() {
        assert_eq!(identity("Dell", "U2720Q", "ABC123", "DP-1", false), "Dell U2720Q ABC123");
    }

    #[test]
    fn without_a_serial_the_connector_tells_them_apart() {
        assert_eq!(identity("BOE", "0x0BCA", "", "eDP-1", false), "BOE 0x0BCA @eDP-1");
    }

    #[test]
    fn an_identical_twin_gets_its_connector() {
        let connected = [screen(1, output("DP-1", "Dell", "U2720Q", "0"))];
        assert_eq!(connected[0].identity, "Dell U2720Q 0");
        let twin = output("DP-2", "Dell", "U2720Q", "0");
        assert_eq!(identity_of(&twin, &connected), "Dell U2720Q 0 @DP-2");
    }

    #[test]
    fn a_floating_screen_comes_back_on_any_port() {
        let floating = [screen(1, output("DP-1", "Dell", "U2720Q", "0"))];
        let back = output("HDMI-A-1", "Dell", "U2720Q", "0");
        let id = identity_of(&back, []);
        assert_eq!(returning(&id, &floating), Some(ScreenId(1)));
        assert_eq!(returning("Samsung X 1", &floating), None, "never seen");
    }

    #[test]
    fn a_returning_monitor_is_what_the_screen_draws_on() {
        // Plugged back in, a monitor is a new `Output`: the screen's space has
        // to hold that one, or nothing renders on it.
        let mut s = screen(1, output("DP-1", "Dell", "U2720Q", "1"));
        let back = output("HDMI-A-1", "Dell", "U2720Q", "1");
        s.set_output(back.clone());
        assert_eq!(s.output, back);
        assert_eq!(s.space.outputs().collect::<Vec<_>>(), vec![&back]);
    }

    #[test]
    fn gpu_renumbering_preserves_a_monitor_without_a_serial() {
        let before = output("card1-Virtual-1", "Red Hat", "QEMU", "");
        before.user_data().insert_if_missing(|| ConnectorId("pci-0000:00:01.0-Virtual-1".into()));
        let floating = [screen(1, before)];
        let back = output("card0-Virtual-1", "Red Hat", "QEMU", "");
        back.user_data().insert_if_missing(|| ConnectorId("pci-0000:00:01.0-Virtual-1".into()));
        assert_eq!(returning(&identity_of(&back, []), &floating), Some(ScreenId(1)));
        let other_gpu = output("card1-Virtual-1", "Red Hat", "QEMU", "");
        other_gpu.user_data().insert_if_missing(|| ConnectorId("pci-0000:00:02.0-Virtual-1".into()));
        assert_eq!(returning(&identity_of(&other_gpu, []), &floating), None);
    }

    #[test]
    fn size_follows_mode_scale_and_rotation() {
        let s = screen(1, output("DP-1", "Dell", "U2720Q", "1"));
        assert_eq!(s.size(), (1280, 720).into(), "no mode yet: a default");
        let mode = Mode { size: (3840, 2160).into(), refresh: 60_000 };
        s.output.change_current_state(Some(mode), None, Some(smithay::output::Scale::Integer(2)), None);
        assert_eq!(s.size(), (1920, 1080).into());
        s.output.change_current_state(None, Some(smithay::utils::Transform::_90), None, None);
        assert_eq!(s.size(), (1080, 1920).into());
    }
}

mod ipc {
    use wm::{
        input::Action,
        ipc::{Command, Request, parse},
        layout::Direction,
    };

    fn ok(line: &str) -> Request {
        parse(line).unwrap_or_else(|e| panic!("{line}: {e}"))
    }

    #[test]
    fn commands() {
        assert_eq!(ok("tree").command, Command::Tree);
        assert_eq!(ok("subscribe").command, Command::Subscribe);
        assert_eq!(ok("focus left").command, Command::Action(Action::Focus(Direction::Left)));
        assert_eq!(ok("move right").command, Command::Action(Action::Move(Direction::Right)));
        assert_eq!(ok("workspace 1").command, Command::Action(Action::Workspace(0)));
        assert_eq!(ok("move-to-workspace 9").command, Command::Action(Action::MoveToWorkspace(8)));
        assert_eq!(ok("animations off").command, Command::Action(Action::Animations(Some(false))));
        assert_eq!(ok("animations").command, Command::Action(Action::Animations(None)));
        assert_eq!(ok("spawn kitty -e  htop").command, Command::Action(Action::Spawn("kitty -e htop".into())));
        assert_eq!(ok("pointer 10 20.5").command, Command::Pointer(10.0, 20.5));
        assert_eq!(ok("click").command, Command::Click(None));
        assert_eq!(ok("click 1 2").command, Command::Click(Some((1.0, 2.0))));
        assert_eq!(ok("  close \n").command, Command::Action(Action::Close));
        assert_eq!(ok("take-workspace 7").command, Command::TakeWorkspace(7));
        assert_eq!(ok("screen-new").command, Command::ScreenNew);
        assert_eq!(ok("screen-remove 2").command, Command::ScreenRemove(2));
        assert_eq!(ok("screen-add 2").command, Command::ScreenAdd(2));
    }

    #[test]
    fn seat_prefix() {
        assert_eq!(ok("focus left").seat, None);
        let r = ok("seat 1 click 5 6");
        assert_eq!(r.seat, Some(1));
        assert_eq!(r.command, Command::Click(Some((5.0, 6.0))));
    }

    #[test]
    fn errors() {
        for line in [
            "", "seat", "seat x tree", "seat 1", "fly", "focus up", "focus", "workspace 0", "workspace 10",
            "workspace x", "animations maybe", "pointer 1", "click 1", "spawn", "tree now",
            "take-workspace", "take-workspace x", "screen-new 1", "screen-remove", "screen-add x",
        ] {
            assert!(parse(line).is_err(), "{line:?} should fail");
        }
    }

    mod requests {
        use std::{
            io::Write,
            os::unix::net::UnixStream,
            thread,
            time::{Duration, Instant},
        };

        use wm::ipc::{MAX_LINE, TIMEOUT, read_request};

        #[test]
        fn a_line() {
            let (mut client, server) = UnixStream::pair().unwrap();
            client.write_all(b"seat 1 tree\nmore").unwrap();
            assert_eq!(read_request(&server).unwrap(), "seat 1 tree");
        }

        #[test]
        fn a_slow_drip_runs_out_of_time() {
            let (mut client, server) = UnixStream::pair().unwrap();
            let drip = thread::spawn(move || {
                for _ in 0..30 {
                    if client.write_all(b"t").is_err() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            });
            let start = Instant::now();
            assert!(read_request(&server).is_err());
            assert!(start.elapsed() < TIMEOUT + Duration::from_millis(200), "took {:?}", start.elapsed());
            drop(server);
            drip.join().unwrap();
        }

        #[test]
        fn an_endless_line_is_cut_off() {
            let (mut client, server) = UnixStream::pair().unwrap();
            client.write_all(&vec![b'x'; MAX_LINE * 2]).unwrap();
            assert!(read_request(&server).is_err());
        }
    }
}

mod events {
    use serde_json::json;
    use wm::ipc::{Snapshot, diff};

    fn snapshot(items: &[(&'static str, u64, serde_json::Value)]) -> Snapshot {
        items.iter().map(|(kind, id, v)| ((*kind, *id), v.clone())).collect()
    }

    #[test]
    fn everything_is_new_at_first() {
        let now = snapshot(&[("seat", 0, json!({"id": 0, "number": 1}))]);
        assert_eq!(diff(&Snapshot::new(), &now), vec![json!({"event": "seat", "change": "new", "id": 0, "number": 1})]);
    }

    #[test]
    fn only_changes_are_told() {
        let old = snapshot(&[
            ("seat", 0, json!({"id": 0, "number": 1})),
            ("seat", 1, json!({"id": 1, "number": 2})),
            ("window", 7, json!({"id": 7, "title": "a"})),
        ]);
        let new = snapshot(&[
            ("seat", 0, json!({"id": 0, "number": 1})),
            ("seat", 1, json!({"id": 1, "number": 4})),
            ("window", 8, json!({"id": 8, "title": "b"})),
        ]);
        assert_eq!(
            diff(&old, &new),
            vec![
                json!({"event": "seat", "change": "change", "id": 1, "number": 4}),
                json!({"event": "window", "change": "new", "id": 8, "title": "b"}),
                json!({"event": "window", "change": "gone", "id": 7}),
            ]
        );
        assert!(diff(&new, &new).is_empty());
    }
}

mod options {
    use smithay::input::keyboard::{Keysym, ModifiersState, keysyms};
    use wm::{Backend, Command, Config, Modifier, input::binding};

    fn args(s: &str) -> Result<Command, String> {
        Config::from_args(s.split_whitespace().map(String::from))
    }

    #[test]
    fn options() {
        assert_eq!(Config::default().backend, Backend::Winit);
        let Ok(Command::Run(c)) = args("--mod super --seats 3 -c kitty") else { panic!() };
        assert_eq!(c.modifier, Modifier::Super);
        assert_eq!(c.seats, 3);
        assert_eq!(c.command.as_deref(), Some("kitty"));
        assert!(matches!(args("--backend drm"), Ok(Command::Run(Config { backend: Backend::Drm, .. }))));
        assert!(matches!(args("--backend winit"), Ok(Command::Run(Config { backend: Backend::Winit, .. }))));
        for bad in ["--seats 0", "--seats", "--mod hyper", "--terminal foot", "--what", "--backend", "--backend tty", "--config"] {
            assert!(args(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn device_configuration_routes_names_and_validates_seats() {
        let path = std::env::temp_dir().join(format!("wm-device-config-{}.json", std::process::id()));
        let parse = |source: &str, seats: &str| {
            std::fs::write(&path, source).unwrap();
            Config::from_args(["--config", path.to_str().unwrap(), "--seats", seats].map(String::from))
        };
        let Ok(Command::Run(c)) = parse(r#"{"devices":{"USB Keyboard":2,"USB Mouse":1}}"#, "3") else { panic!() };
        assert_eq!(c.device_seat("USB Keyboard"), 2);
        assert_eq!(c.device_seat("USB Mouse"), 1);
        assert_eq!(c.device_seat("usb mouse"), 0);
        assert_eq!(c.device_seat("unconfigured"), 0);
        for bad in [
            r#"{"devices":{"USB Keyboard":3}}"#,
            r#"{"devices":{"USB Keyboard":-1}}"#,
            r#"{"devices":{"USB Keyboard":"1"}}"#,
            r#"{"devices":{"USB Keyboard":1.5}}"#,
            r#"{"devices":{" ":1}}"#,
            r#"{"devices":[]}"#,
            r#"{"device":{}}"#,
            "[]", "invalid",
        ] {
            assert!(parse(bad, "3").is_err(), "{bad}");
        }
        assert!(parse("{}", "1").is_ok());
        std::fs::remove_file(&path).unwrap();
        assert!(Config::from_args(["--config".into(), path.to_string_lossy().into_owned()]).is_err());
    }

    #[test]
    fn msg_passes_the_rest_through() {
        assert_eq!(args("msg seat 1 focus left"), Ok(Command::Msg(vec!["seat".into(), "1".into(), "focus".into(), "left".into()])));
    }

    fn mods(alt: bool, logo: bool, shift: bool) -> ModifiersState {
        ModifiersState { alt, logo, shift, ..Default::default() }
    }

    #[test]
    fn no_bindings_yet() {
        // Every key goes to the client, with the modifier or without.
        for m in [mods(true, false, false), mods(true, false, true), mods(false, false, false)] {
            for sym in [keysyms::KEY_Return, keysyms::KEY_Left, keysyms::KEY_3, keysyms::KEY_q] {
                assert_eq!(binding(Modifier::Alt, &m, Keysym::new(sym)), None);
            }
        }
    }
}

mod input {
    use wm::input::{clamp_pointer, virtual_terminal};

    #[test]
    fn vt_shortcuts_belong_to_drm() {
        use smithay::input::keyboard::{Keysym, ModifiersState, keysyms};
        use wm::Backend;
        let mods = ModifiersState { ctrl: true, alt: true, ..Default::default() };
        for (sym, vt) in [(keysyms::KEY_F1, 1), (keysyms::KEY_F12, 12)] {
            assert_eq!(virtual_terminal(Backend::Drm, &mods, Keysym::new(sym)), Some(vt));
            assert_eq!(virtual_terminal(Backend::Winit, &mods, Keysym::new(sym)), None);
        }
        assert_eq!(virtual_terminal(Backend::Drm, &mods, Keysym::new(keysyms::KEY_F13)), None);
        assert_eq!(virtual_terminal(Backend::Drm, &ModifiersState::default(), Keysym::new(keysyms::KEY_F1)), None);
    }

    #[test]
    fn keyboards_keep_seats_and_sources_separate() {
        use smithay::{backend::input::{InputTime, KeyState}, input::keyboard::{KeyboardSource, Keysym, keysyms}, reexports::{calloop::EventLoop, wayland_server::Display}};
        use wm::{Backend, Config, state::State};
        // Give this backend-free State its own socket directory without
        // changing the environment of parallel tests.
        if std::env::var_os("WM_TEST_INPUT_CHILD").is_none() {
            use std::os::unix::fs::PermissionsExt;
            let runtime = std::env::temp_dir().join(format!("wm-input-{}", std::process::id()));
            std::fs::create_dir(&runtime).unwrap();
            std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "input::keyboards_keep_seats_and_sources_separate", "--nocapture"])
                .env("WM_TEST_INPUT_CHILD", "1").env("XDG_RUNTIME_DIR", &runtime).output().unwrap();
            std::fs::remove_dir_all(runtime).unwrap();
            assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            return;
        }
        let mut event_loop = EventLoop::try_new().unwrap();
        let mut state = State::new(&mut event_loop, Display::new().unwrap(), Config { backend: Backend::Drm, ..Default::default() });
        let keyboard = state.seats[0].seat.get_keyboard().unwrap();
        let key = |sym| keyboard.keycode_for_keysym(Keysym::new(sym)).unwrap();
        let source = KeyboardSource::new_auxiliary();
        let other = KeyboardSource::new_auxiliary();
        let time = InputTime::from_millis(1);
        let (ctrl, alt, f1, a) = (key(keysyms::KEY_Control_L), key(keysyms::KEY_Alt_L), key(keysyms::KEY_F1), key(keysyms::KEY_a));
        for code in [ctrl, alt] { assert_eq!(state.keyboard_input(0, source, code, KeyState::Pressed, time), None); }
        assert_eq!(state.keyboard_input(0, source, f1, KeyState::Pressed, time), Some(1));
        assert!(state.seats[0].intercepted_keys.contains(&f1));
        for code in [ctrl, alt, f1] { assert_eq!(state.keyboard_input(0, source, code, KeyState::Released, time), None); }
        assert!(state.seats[0].intercepted_keys.is_empty());
        assert!(keyboard.pressed_keys().is_empty());
        for device in [source, other] { state.keyboard_input(0, device, a, KeyState::Pressed, time); }
        keyboard.release_source(&mut state, source);
        assert!(keyboard.pressed_keys().contains(&a));
        assert!(state.seats[1].seat.get_keyboard().unwrap().pressed_keys().is_empty());
        keyboard.release_source(&mut state, other);
        assert!(keyboard.pressed_keys().is_empty());
        state.config.backend = Backend::Winit;
        for code in [ctrl, alt, f1] { assert_eq!(state.keyboard_input(1, source, code, KeyState::Pressed, time), None); }
        assert!(state.seats[1].intercepted_keys.is_empty());
    }

    #[test]
    fn physical_pointer_stays_inside_each_screen() {
        let size = (1280, 720).into();
        assert_eq!(clamp_pointer((-10.0, 900.0).into(), size), (0.0, 719.0).into());
        assert_eq!(clamp_pointer((1400.0, -1.0).into(), size), (1279.0, 0.0).into());
        assert_eq!(clamp_pointer((100.5, 200.25).into(), size), (100.5, 200.25).into());
        assert_eq!(clamp_pointer((10.0, 20.0).into(), (1, 1).into()), (0.0, 0.0).into());
    }
}


mod dmabuf {
    use smithay::{backend::{allocator::{Format, Fourcc, Modifier}, drm::DrmNode}, reexports::{rustix::fs::makedev, wayland_server::Display}};
    use wm::{dmabuf::DmabufSupport, state::State};

    #[test]
    fn gpu_global_is_withdrawn_and_recreated_on_return() {
        let display = Display::<State>::new().unwrap();
        let dh = display.handle();
        let mut support = DmabufSupport::default();
        let node = DrmNode::from_dev_id(makedev(226, 128)).unwrap();
        let format = Format { code: Fourcc::Argb8888, modifier: Modifier::Linear };
        assert!(support.enable(&dh, 1, Some(node), []).unwrap().is_none());
        assert_eq!(support.device, None, "no formats means no usable import device");
        let original = support.enable(&dh, 1, Some(node), [format]).unwrap().unwrap();
        assert_eq!(support.enable(&dh, 2, None, [format]).unwrap(), Some(original));
        assert_eq!(support.device, Some(1), "a second GPU must not replace a live global's importer");
        assert_eq!(support.disable(&dh), Some(original));
        assert_eq!(support.device, None);
        assert!(support.disable(&dh).is_none());
        let returned = support.enable(&dh, 2, Some(node), [format]).unwrap().unwrap();
        assert_ne!(returned, original, "returning GPUs publish fresh bindings");
        assert_eq!(support.device, Some(2));
        support.state.destroy_global::<State>(&dh, original);
        let live = support.disable(&dh).unwrap();
        support.state.destroy_global::<State>(&dh, live);
    }
}
