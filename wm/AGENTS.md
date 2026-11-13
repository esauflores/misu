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
| `src/lib.rs` | module list, command-line options |
| `src/state.rs` | State: seats, screens, workspaces, windows, focus |
| `src/layout.rs` | columns, per-seat views, `Anim` |
| `src/ipc.rs` | socket, `wm msg`, `tree`, events |
| `src/input.rs` | input routing, `Action`, `binding()` (empty on purpose) |
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
launch tracking, the control socket with `tree` and `subscribe`. Tests: 43
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
the AI dot. Step 3 is next.

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
3. **Key bindings and a config file**: fill `binding()`, read Misu's
   bindings (Super+n, Super+arrows, …) from a file, plus the options that are
   flags now.
4. **Drawing an unseen seat**: render a seat's workspace off screen for
   screencopy, so an agent can screenshot its own workspace (what
   `misu-ai-shot` does with scroll).
5. **Screencopy and portals** (`ext-image-copy-capture`, wlr-screencopy,
   xdg-desktop-portal-wlr) for grim and screen sharing; **XWayland**;
   popup grabs per seat (`TODO` in `handlers.rs`); server-side decorations;
   column widths and full screen.
6. **Port Misu**: the session entry (`misu-session`), the `misu-ai-*`
   scripts to `wm msg seat N …` (most of their workarounds go away), the
   pointer plugin to wm's own seat pointers, and a `wm` module in place of
   scroll's.
