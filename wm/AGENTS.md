# Agent notes

wm is a scrolling Wayland compositor in Rust on smithay, written to replace
scroll (sway-scroll) in Misu (`/opt/misu`). scroll has one global
`animations_enable`, and another seat refocusing restarts its animations, so
several seats and animations don't work together there; Misu's `misu-ai-*`
scripts are a pile of workarounds for that. wm is built around both: several
seats, so several people (or agents) can work at once, with animations per
seat.

`README.md` describes what it does and how to run it; this file is how to
work on it, and the plan.

## Working on it

```sh
mise install                                   # Rust, pinned in mise.toml
/usr/bin/mise exec -- cargo test               # unit + e2e
/usr/bin/mise exec -- cargo clippy --all-targets
```

- `cargo test` must pass and clippy must be clean after every change. Add a
  test for each fix: `tests/unit.rs` (a module per subject) or a step in
  `tests/e2e.rs` driven through `wm msg`.
- `tests/e2e.rs` runs wm nested in a headless `scroll` with kitty windows,
  nothing on screen. Don't start wm any other way from an agent: nested, it
  opens a window on the user's screen.
- `references/` (git-ignored) has niri, smithay (the pinned revision),
  cosmic-comp, scroll, sway, wlroots and Hyprland. Check smithay semantics and
  how niri solves a problem there before inventing.
- Keep it small: few files, few lines, no features beyond what's asked.
- Never commit or push until the user has checked the change and asked for
  it, every time.
- Explain it as several people working at once, not as an "AI desk".

## Design (decided; change only for a concrete flaw)

- **Seats** are users. Each has its own pointer, keyboard focus, workspace,
  scroll view of each workspace, animation setting and colour. Seats not on a
  screen keep working there. Nested, the host's input drives one seat
  (`next-seat` switches).
- **Animations are per seat**: a seat's changes animate when it has them on
  and a screen shows it. An `Anim` retargets from its current value and never
  restarts.
- **Columns are as wide as the screen** (`DEFAULT_WIDTH` 1.0): one app per
  screen, scrolled between. A toplevel with a parent (xdg `set_parent`: a
  dialog) floats centred over it at its own size (`Workspace::floats`),
  follows it in every seat's view, and gives focus back to it on close.
  Apps opened by other apps still tile; floating them too was option 2,
  left for now.
- **Full screen** is a column's: its window takes the whole screen
  (`Workspace::screen`, not the area the bar leaves) from the screen's
  corner when a view has the column at its edge, and the column's room is
  the screen less the gaps, so neighbours stay off screen. The seat that
  asks has its view snapped there. While a seat's view is covered
  (`Workspace::covered`), Top layer surfaces (the bar) are neither drawn
  nor hit; Overlay ones still are. Every backend draws a screen through
  `State::screen_elements` (shared with screenshots and capture), not the
  `Space`, which would put Top layers over it.
- **Menus** (xdg popup grabs) are per seat: the client names the seat
  (`grab`'s `wl_seat`), and only that seat's pointer and keyboard are
  held (`SeatData::menu`). Smithay closes a menu on a click on another
  client's surface; one inside the same app is the app's (GTK closes its
  popover). A closed menu's grab is let go each frame (`end_menus`), not
  with the seat's next event, and a seat's focus moving elsewhere (window,
  workspace, layer surface) closes its menus (`update_keyboard`).
- **Clicks** focus without scrolling; keyboard and explicit moves reveal the
  window in that seat's view.
- **Screens** have an incrementing `ScreenId`; make, model and serial (plus
  the connector for twins or no serial) only recognise a returning monitor.
  Unplugged screens float with their workspaces. A screen has an optional
  `viewer` seat and shows its workspace (else its first). Each screen has its
  own `Space`.
- **Workspaces** have stable ids in one HashMap; each screen holds an ordered
  Vec of ids, made on demand.
- **Launching**: `spawn` sets `WM_LAUNCH_SEAT`, read back from
  `/proc/<client pid>/environ`, so a window opens on its launcher's
  workspace, even through `setsid -f`.
- **Control socket** (`$XDG_RUNTIME_DIR/wm.<wayland socket>.sock`, path in
  `$WM_SOCK` for clients): one plain-text line in, one JSON line out.
  `subscribe` keeps the connection and streams events, found by diffing a
  snapshot of seats, screens and windows after each turn of the loop.
- **One thread**: calloop serialises everything, so seats can't race.

## Where things are

| File | What |
|---|---|
| `src/main.rs` | startup |
| `src/lib.rs` | module list, options and the config file |
| `src/state.rs` | State: seats, screens, workspaces, windows, focus |
| `src/layout.rs` | columns, per-seat views, `Anim` |
| `src/ipc.rs` | socket, `wm msg`, `tree`, events |
| `src/input.rs` | input routing, `Action` |
| `src/bindings.rs`, `config.json` | key combos, default bindings |
| `src/shot.rs` | a seat's view drawn off screen, as a PNG |
| `src/capture.rs` | ext-image-copy-capture, foreign toplevel list |
| `src/winit.rs` | nested backend and rendering |
| `src/udev.rs` | DRM/libinput backend, libseat lifecycle, GPU/output hotplug |
| `src/dmabuf.rs` | linux-dmabuf: formats, deferred imports |
| `src/layers.rs` | layer shell: bars, launchers, their room and focus |
| `src/handlers.rs` | Wayland protocol handlers |
| `tests/unit.rs`, `tests/e2e.rs` | tests |

## Status

Works nested (winit): several seats, per-seat views and animations, focus
and moves, workspaces made on demand, move-to-workspace, close, screens
(virtual ones through `screen-new`), unplug and replug, `take-workspace`,
launch tracking, the control socket with `tree` and `subscribe`. Tests: 46
unit plus the e2e run; clippy clean.

Step 1 implementation: explicit `--backend drm` (winit remains the default),
libseat session pause/resume and VT switching, libinput devices mapped by
exact name through `--config FILE`, GPU/connector hotplug, and per-output DRM
compositors with damage tracking. Validated in Misu's KVM guest with virtio-gpu:
kitty, keyboard, absolute/relative pointers, seat mapping, two outputs,
input/connector/GPU hotplug, all outputs disconnected, VT recovery, idle
damage and clean shutdown. GPU connector keys use stable device paths, so
DRM card renumbering preserves screen IDs. Live mode changes keep the screen
ID (VM: 1366x768 to 1024x768 and back), and at poweroff wm stops before
seatd. linux-dmabuf client buffers (`src/dmabuf.rs`) work: the e2e run checks
kitty's Mesa dmabufs are imported on this laptop's Intel GPU. wm targets this
laptop: one Intel GPU drives the panel and every port, so multi-GPU is out of
scope (hotplug of a GPU is handled, rendering across two is not).

Step 2: layer shell (`src/layers.rs`). Layer surfaces belong to a screen;
their exclusive zones shrink its workspaces' area (`Screen::area`, offset
in `Workspace::positions`/`targets`). Keyboard focus for them is per seat
(`SeatData::layer_focus`): `Exclusive` (fuzzel) goes to the launching seat
and survives clicks on windows; `OnDemand` goes to the clicking seat. The
e2e run checks a Quickshell bar's room and fuzzel's focus; Misu's real bar
runs in wm, and its `services/Wm.qml` follows `wm msg subscribe` when
`$WM_SOCK` is set (scroll otherwise). Not yet: layer popups' grabs, a bar
per seat (one screen's layers are everyone's on it), Misu's `misu-ai-*` and
the AI dot.

Step 3: bindings and a config file. `Config::load` takes the defaults
(`config.json`, compiled in), then the file (`--config`, else
`~/.config/wm/config.json` if there), then the flags. Keys: `modifier`,
`seats`, `startup`, `devices`, `bindings`. A binding's value is `wm msg`
commands joined by `;`, run for the pressing seat (`ipc::parse`, so one
grammar); `null` unbinds. Matching is exact on Ctrl/Alt/Shift/Super with the
unshifted keysym. `Mod` is Super on DRM and Alt nested. New commands: `focus
first|last`, `move-to-workspace N follow`, `reload` (seats and backend stay).
Misu's own bindings file comes with step 6; what it binds that wm lacks
(floating, column sizes, `jump`) is step 5.

Step 4: `wm msg seat N screenshot FILE` draws seat N's own view (its
scroll position, its pointer and mark, its screen's layer surfaces) into an
offscreen texture with the next frame and writes a PNG; the reply waits
for it (`State::shots`, taken in each backend's frame by
`State::take_shots`; refused while the session is paused). `wm msg` makes
the path absolute. The e2e run shoots seat 1 while no screen shows it and
checks its window is in the picture.

Step 5 so far: full-width columns and floating dialogs (above); the e2e
run opens a GTK 4 window and a dialog for it (python3 with GTK 4).
Capture (`capture.rs`): `ext-image-copy-capture` with screen and window
sources and `ext-foreign-toplevel-list`, whose identifiers are the tree's
window ids. grim 1.5, hyprpicker 0.4 and xdg-desktop-portal-wlr 0.8 all
speak it, so there is no wlr-screencopy. A screen's capture is its
viewer's view (`State::view_elements`, shared with `shot.rs`), pointers
only when asked; a window's is the window alone at its geometry. Frames
are drawn with the backend's next frame into shm (BGRA from a GL
readback; no dma-buf targets yet) once something changed (a damage
tracker per session); a resize sends new constraints and fails the
frame. The e2e run checks `grim -o` against seat 0's screenshot and
`grim -T` on seat 1's unseen window. Screen sharing through the portal
is unchecked: it needs the session's portal setup (step 6).
Full screen (above): `wm msg fullscreen [on|off|toggle]`, `Mod+f`, and
xdg `set_fullscreen` (before the window shows too); the e2e run covers
the test bar and clicks through where it was.
Menus (above): the e2e run opens a GTK popover, checks seat 0 alone is
held, clicks outside it and checks the grab is gone and the window has the
keyboard back. Closing on a focus change is untested.
Left of step 5: XWayland, server-side decorations.

Known and left for now:

- The pinned Smithay libseat notifier unwraps dispatch errors: losing seatd
  can panic (observed during guest poweroff with wm still active). Stop wm
  before seatd; orderly IPC quit passes. Fixing daemon-loss handling needs
  a Smithay change.
- An unseen seat's view of a workspace can animate for 250 ms when a window
  closes or the screen resizes there (`Workspace::arrange` clamps all views
  with the shown seat's flag); fixing it needs the viewer in the layout.
- Nested, the host-driven seat can end up on a virtual screen nobody sees.
- `cargo fmt --check` fails (144 spots, long one-line `json!` calls): decide
  between `cargo fmt` and a `rustfmt.toml` with a wider `max_width`.

## Plan: what's missing to replace scroll

In order; each step tested in Misu's KVM VM (`/opt/misu/test/vm.sh`, see
Misu's `AGENTS.md`) before the next.

1. **DRM/libinput backend (implemented; KVM validated)**: real screens and hotplug (screen ids and
   floating already handle it), libinput devices assigned to seats (one seat
   by default; a config line puts a keyboard or mouse on another), a session
   through libseat, rendering per output with damage tracking instead of a
   full redraw each frame.
2. **Layer shell (implemented; nested e2e and the real bar checked)**, so
   Quickshell (the bar, the greeter) and fuzzel run. Then port the bar's
   `services/Wm.qml` to `wm msg subscribe`.
3. **Key bindings and a config file (implemented)**: bindings (Super+n,
   Super+arrows, …) from a file, plus the options that are flags now.
4. **Drawing an unseen seat (implemented)**: render a seat's workspace off
   screen, so an agent can screenshot its own workspace (what
   `misu-ai-shot` does with scroll). Screencopy for it is step 5.
5. **Screencopy and portals** (`ext-image-copy-capture`, implemented;
   xdg-desktop-portal-wlr) for grim and screen sharing; **XWayland**;
   popup grabs per seat (implemented); server-side decorations;
   full screen (implemented).
6. **Port Misu**: the session entry (`misu-session`), the `misu-ai-*`
   scripts to `wm msg seat N …` (most of their workarounds go away), the
   pointer plugin to wm's own seat pointers, and a `wm` module in place of
   scroll's.
