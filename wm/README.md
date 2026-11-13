# wm

A scrolling Wayland compositor on [smithay](https://github.com/Smithay/smithay),
built around several seats on one screen:

- **Each seat is its own user**: its own pointer, keyboard focus, workspace,
  and scroll position, so several people can work at once. One seat is on
  screen; the others keep working on their own workspaces whether anyone sees
  them or not. Two seats on the same workspace each scroll their own view.
- **One app per screen**: each window gets a column as wide as the screen,
  and a workspace is a row of them to scroll along. A window an app opens
  for another of its windows (a dialog, a file picker) floats centred over
  that one at its own size instead. Full screen (`Mod+f`, or the app
  asking) a window covers the whole screen, bar and all. Apps leave
  their title bars to wm (GTK 4 ones keep theirs), and wm draws a 2px
  border round each window: the colour of the seat focusing it, from
  the config file's `colors`. X11 apps run through xwayland-satellite
  (when it is installed) as ordinary windows. A menu holds
  the pointer and keyboard of the seat that opened it, and only that
  seat's.
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

Keys are bound in a JSON config file, `~/.config/wm/config.json` (or
`--config FILE`), over wm's own defaults in [`config.json`](config.json):
Mod+Return a terminal, Mod+f full screen, Mod+Left/Right focus, Mod+Shift+Left/Right move,
Mod+1…9 workspaces, Mod+Shift+1…9 take the window there, Mod+Tab the next
seat (nested), Mod+Ctrl+Shift+R reload. Nothing quits
wm from the keyboard: `wm msg quit` does. `Mod` is
Alt nested (the host keeps Super) and Super on a VT; `--mod` or
`"modifier"` changes it.

```json
{
  "modifier": "super",
  "seats": 2,
  "startup": ["qs -c misu"],
  "devices": {"USB Keyboard": 1},
  "bindings": {
    "Mod+Return": "spawn foot",
    "Mod+Shift+q": "close; focus left",
    "Mod+w": null
  }
}
```

A binding runs `wm msg`'s commands (`;` between them) for the seat whose
keyboard pressed it; `null` unbinds a default. Keys are xkb keysym names, in
any case; `Shift+2` is written with `2`. Command-line flags go over the
file, and `reload` (or Mod+Ctrl+Shift+R) reads it again. `input` sets the
keyboard (`layout`, `variant`, `options`, `repeat_rate`, `repeat_delay`;
every seat's), touchpads and mice (`tap`, `drag`, `natural_scroll`, `dwt`,
`click_method`, `accel_speed`; libinput, on a VT) and
`focus_follows_mouse`; see `src/settings.rs`. `screens` (on a VT) lists
monitors by connector (`eDP-1`) or identity, each with an optional `mode`
(`"1366x768@60"`) and `off`; they go left to right in the list's order. A
laptop's panel turns off while its lid is closed. See `src/screens.rs`.

Each seat on the shown workspace has its pointer drawn with a real cursor:
the app's (its own surface, or a shape it names), else the theme's arrow,
from `XCURSOR_THEME` and `XCURSOR_SIZE`. Seat 0 has the theme as it is;
the others have it tinted their colour. The window a seat focuses has a
border in that colour. Clicking
focuses without scrolling. Seats start on workspaces of their own (seat0 on
1, seat1 on 2, …), and each screen shows one seat's.

## Run on a VT (DRM/libinput)

The direct backend uses libseat for device access, udev for GPU and monitor
hotplug, libinput for devices, and a damage-tracked DRM compositor per output.
Winit remains the default, so existing nested runs and headless tests keep
using it. The direct backend has been validated in Misu's KVM guest with
virtio-gpu.

Build dependencies on Arch include `libinput`, `seatd` (libseat), `libdisplay-info`,
`libdrm`, `mesa`, `libglvnd`, `systemd-libs` and `pkgconf`. From an active VT
login with a logind session (or a configured seatd):

```sh
wm --backend drm -c kitty
```

Devices use seat 0 unless their exact libinput name is in the config file's
`devices` (`{"USB Keyboard":1,"USB Mouse":1}`). Names and assigned seats are
logged when devices connect; seat indices start at zero and must be below
the number of seats. On DRM, Ctrl+Alt+F1 through F12 switch VTs.
Session pause and device removal release held input, and returning to the VT
restores scanout. Monitor unplug preserves its screen number and workspaces.
Clients can hand over GPU buffers through linux-dmabuf (as Mesa's EGL does) as
well as `wl_shm`.

Bars, launchers and backgrounds (Quickshell, fuzzel) use the layer shell.
Room a bar keeps for itself is taken off its screen's workspaces; `wm msg
tree` shows each screen's `area` and `layers`. A launcher that wants the
keyboard gets the keyboard of the seat that started it (`layer_focus` in
the tree), and gives it back to that seat's window when it closes.

VM checks cover kitty scanout, keyboard and absolute/relative pointer input,
device assignment to separate seats, two outputs, input/connector/GPU
unplug and replug, all outputs disconnected, VT recovery, and clean shutdown.
A regression test covers GPU card renumbering without changing screen IDs.
Blank output submitted zero frames over a two-second idle check. With
`RUST_LOG=wm=trace`, submissions appear as `DRM frame queued`. A live mode
change keeps the screen number. wm is made for one GPU driving every screen;
rendering across two GPUs isn't supported. The nested e2e test does not
exercise the direct backend, but it does check kitty's dmabufs are imported.

## Control socket

The socket is `$XDG_RUNTIME_DIR/wm.<wayland socket>.sock`. Everything
started inside wm gets its path in `$WM_SOCK`; `wm msg` talks to it and
prints one JSON line (`{"ok":true,"result":…}` or `{"ok":false,"error":…}`).

```sh
wm msg tree                    # windows, workspaces, seats
wm msg focus right
wm msg seat 1 click 300 200    # any seat; without `seat N`, the one the host drives
wm msg seat 1 screenshot a.png # what seat 1 sees, as a PNG, on screen or not
wm msg animations off
wm msg screen-new              # a virtual screen; screen-remove N / screen-add N unplug and replug
wm msg subscribe               # stays open: the state, then a line per change
wm msg help                    # the rest
```

Other programs capture through `ext-image-copy-capture`: a screen as it
shows (`grim -o NAME`), or a window alone, wherever it is and whether or
not anyone sees it (`grim -T ID`, with its id from `wm msg tree`).
hyprpicker and xdg-desktop-portal-wlr use it too.

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
grim capturing the screen and an unseen window, full screen over a bar,
a dialog floating over its window, a menu's grab, an X11 app and its
dialog, workspaces, closing, reaping, a second screen that draws only its own
windows, unplugging one (its workspace floats, its viewer moves), taking a
floating workspace over, plugging it back, a new screen for an unseen seat,
quitting, and that a wm inside another leaves the outer socket alone. It is skipped without `scroll` and `kitty`, and leaves
logs and a screenshot in `target/tmp/e2e/`.

## Layout

- `src/main.rs`: starts it; the rest is a library for the tests
- `src/state.rs`: compositor state: seats, screens (their ids, and
  recognising a monitor), workspaces, windows, focus
- `src/layout.rs`: columns, each seat's view of them, and animated values
  that retarget instead of restarting
- `src/ipc.rs`: the control socket, `wm msg`, and what `subscribe` hears
- `src/lib.rs` and `config.json`: options, the config file, default bindings
- `src/bindings.rs`: key combos and the commands they run
- `src/input.rs`: input routing, and the actions bindings and `wm msg` run
- `src/shot.rs`: screenshots of a seat's own view, drawn off screen
- `src/capture.rs`: capture for other programs (grim, screen sharing)
- `src/decorations.rs`: no title bars from apps; borders, and the colours
- `src/cursor.rs`: each seat's cursor, from the theme or the app
- `src/settings.rs`: input settings: keyboard layout and repeat, touchpad
  and mouse options, focus following the mouse
- `src/xwayland.rs`: X11 apps, through xwayland-satellite
- `src/winit.rs`: the nested backend and rendering
- `src/udev.rs`: the direct DRM backend, session lifecycle, hotplug and devices
- `src/layers.rs`: the layer shell: bars and launchers, their room and focus
- `src/handlers.rs`: Wayland protocol handlers
- `tests/unit.rs`, `tests/e2e.rs`: see Tests

## Next

- Screen sharing checked through the portal, dma-buf capture, a
  sway-compatible IPC
- Rounded corners and shadows (Misu's look under scroll)
