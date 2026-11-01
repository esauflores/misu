---
name: add-bar-plugin
description: Add a widget to the Misu Quickshell bar (a rail button and optional slide-out card). Use when adding a new indicator, control, or card to the bar in modules/bar.
---

# Add a bar plugin

Plugins live in `modules/bar/plugins/<kind>/`, one folder each. Read
`modules/bar/README.md` first, and an existing plugin of similar shape
(`battery` for a button + card + service, `quicksettings` for a button only).

## Files

- `Rail.qml`: the rail button. Has `edge`, `scale`, `widgetId` properties and a
  `menuRequested(string id, rect ownerRect)` signal; wraps a `RailButton`
  (`import "../../rail"`) and emits `menuRequested("<kind>", ...)` on click.
- `Popout.qml` (optional): the card. Has `s` (scale) and `open` properties;
  built from the shared pieces in `modules/bar/popouts/`. Tall content scrolls
  on its own (FrameSurface caps cards at `Config.cardMaxHeight`).
- A service singleton (optional), e.g. `Foo.qml` with `pragma Singleton`,
  polling a CLI through `Quickshell.Io.Process` or a D-Bus API. Backends stay
  with their plugin; other plugins import the folder (`import "../foo"`).

## Wiring

1. Register it in `modules/bar/plugins/index.js`:
   `"<kind>": { rail: ["<rail-id>"], popout: { minWidth: 280 } }`
   (omit `popout` for a button only; `keyboard: "ondemand"` if the card has a
   text field).
2. Add `import "../plugins/<kind>" as Plugin<Kind>` in
   `modules/bar/frame/FrameSurface.qml` next to the others.
3. Place the rail id in a zone: the default in `modules/bar/services/Config.qml`
   (`defaultRail`), or per user in `~/.config/misu/config.json`.
4. Packages the plugin needs at runtime go in `modules/bar/packages.txt`
   (see the `add-module` skill). Mention new binaries in the README's runtime
   dependencies and the plugin list.

## Style

Colors from `Theme`, sizes from `Config`/`s`, icons via
`MaterialIcon { name: "<material symbol>" }`, motion from `Motion`. Match the
comment style of neighbouring files: a short header comment saying what the
file draws or serves.

## Check

The bar config is linked, so the running bar picks up edits after a reload:
`qs kill -c misu; qs -c misu` in the session, or in the VM over SSH (see the
`test-in-vm` skill). Open the card with
`qs -c misu ipc call menu toggle <kind>` and screenshot with `grim`.
