# wm

A scrolling Wayland compositor on [smithay](https://github.com/Smithay/smithay),
built around several seats on one screen:

- **Each seat is its own user**: its own pointer, keyboard focus, workspace,
  and scroll position, so several people can work at once. One seat is on
  screen; the others keep working on their own workspaces whether anyone sees
  them or not. Two seats on the same workspace each scroll their own view.
- **Animations are per seat**: a seat's changes animate when it has them on
  and it is the one on screen; everything else happens at once. A change in
  the middle of an animation carries on from where it is, never restarts.
- **Screens are numbered** (1, 2, …) and recognised by make, model and serial
  (plus the connector for identical twins or a missing serial), so a monitor
  plugged back in, into any port, gets its number and workspaces back.
  Unplugged, a screen floats out of sight with its workspaces and windows as
  they were; whoever was looking at it moves to a screen left, seats working
  there unseen stay, and `take-workspace` brings a floating workspace over.
  Each screen shows the workspace of the seat looking at it, always one of
  its own, drawn with that seat's view; a screen nobody looks at shows its
  first. A new screen goes to a seat no screen shows, whose workspace comes
  over with it, and each screen draws only its own windows. Workspaces
  have ids that never change, in one map; each screen holds the order of its
  own, made on demand (Mod+5 on a screen with three makes the fourth and fifth).
- **Windows open where they were launched**: on the workspace of the seat
  whose command started them, which wm marks in the environment
  (`WM_LAUNCH_SEAT`) its processes inherit, through any double fork. A window
  joins the layout when it first shows something, and leaves it when it
  unmaps.

`references/` (git-ignored) has the sources it learns from: niri (scrolling
layout and animations on smithay), smithay, cosmic-comp, scroll, sway,
wlroots, Hyprland.

## Run (nested in the current compositor)

```sh
mise install                  # Rust, pinned in mise.toml
cargo run -- -c kitty         # opens a window; kitty starts inside it
cargo run -- --seats 3 --mod super
```

There are no key bindings yet; add them in `binding()` in `src/input.rs`
(`--mod` picks their modifier). Until then, drive it with `wm msg` (below).

Each seat on the shown workspace has its pointer drawn as a square in its
colour, and a bar of that colour over the window it has focused. Clicking
focuses without scrolling. Seats start on workspaces of their own (seat0 on
1, seat1 on 2, …), and each screen shows one seat's.

## Control socket

The socket is `$XDG_RUNTIME_DIR/wm.<wayland socket>.sock`. Everything
started inside wm gets its path in `$WM_SOCK`; `wm msg` talks to it and
prints one JSON line (`{"ok":true,"result":…}` or `{"ok":false,"error":…}`).

```sh
wm msg tree                    # windows, workspaces, seats
wm msg focus right
wm msg seat 1 click 300 200    # any seat; without `seat N`, the one the host drives
wm msg animations off
wm msg screen-new              # a virtual screen; screen-remove N / screen-add N unplug and replug
wm msg subscribe               # stays open: the state, then a line per change
wm msg help                    # the rest
```

## Tests

```sh
cargo test                       # everything
cargo test --test unit           # the unit tests
cargo test --test unit layout::  # one subject of them
cargo test --test e2e
```

`tests/unit.rs` holds the unit tests, a module per subject: the layout
(with fake windows), animations, screens, the socket's commands and events,
and the options. `tests/e2e.rs` runs wm
nested in a headless scroll (nothing on screen) with kitty windows and checks
it through the socket: focus and scrolling, a second seat launching on its own
workspace off screen (through a forking launcher), two seats with their own
views of one workspace, clicks after a scroll, a seat's animations on and off,
workspaces, closing, reaping, a second screen that draws only its own
windows, unplugging one (its workspace floats, its viewer moves), taking a
floating workspace over, plugging it back, a new screen for an unseen seat,
quitting, and that a wm inside another leaves the outer socket alone. It is skipped without `scroll` and `kitty`, and leaves
logs and a screenshot in `target/tmp/e2e/`.

## Layout

- `src/main.rs`: starts it; the rest is a library for the tests
- `src/lib.rs`: the modules, and the command-line options
- `src/state.rs`: compositor state: seats, screens (their ids, and
  recognising a monitor), workspaces, windows, focus
- `src/layout.rs`: columns, each seat's view of them, and animated values
  that retarget instead of restarting
- `src/ipc.rs`: the control socket, `wm msg`, and what `subscribe` hears
- `src/input.rs`: bindings, input routing, and the actions they run
- `src/winit.rs`: the nested backend and rendering
- `src/handlers.rs`: Wayland protocol handlers
- `tests/unit.rs`, `tests/e2e.rs`: see Tests

## Next

- A DRM/libinput backend for real hardware, with devices assigned to seats
- Layer shell (the bar), screencopy, portals, a sway-compatible IPC
- Column widths, full screen, popups and grabs per seat
- Rendering a seat that isn't shown, for its screenshots (screencopy)
- Server-side decorations (kitty draws its own title bar for now)
