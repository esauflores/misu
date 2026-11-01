# misu

A Quickshell bar for scroll (sway fork): a single ink rail on the right edge, a card that
slides out of the rail for each widget, and misu's own notification popups.

```sh
qs -c misu          # or: quickshell -p path/to/misu
```

## Layout

```
shell.qml              entry: a Frame per screen, notification and OSD popups, IPC handlers
MaterialIcon.qml       the only icon primitive (Material Symbols)
BrandMark.qml          the 力 seal (config.json markText / markImage)
frame/                 one monitor's bar
  Frame.qml            exclusive zone (FrameEdge) + overlay (chrome, rail, cards)
  FrameEdge.qml        reserves the rail's width so tiled windows stay clear
  FrameChrome.qml/.js  the painted band around the screen hole
  CardManager.qml      which card is open; input mask; one FrameSurface per plugin
  FrameSurface.qml     loads plugins/<kind>/Popout.qml into the shared Popout
rail/                  the strip: layout, widget host, button, tray, workspaces
plugins/<kind>/        one feature per folder: Rail.qml (rail button), Popout.qml (card)
                       and the service for its backend (notifications also: Popups.qml)
plugins/index.js       plugin registry: rail ids → kinds, card widths, keyboard mode
popouts/               the card shell (Popout.qml) and pieces shared by several cards
services/              shell-wide singletons: Theme, Config, Motion, Wm,
                        ShellState and Feedback (OSD state)
plugins/osd/           passive Ito-style volume, brightness and microphone display
```

Plugins: `audio`, `battery`, `bluetooth`, `calendar` (clock),
`aiusage` (every AI plan via the `ai-usagebar` CLI), `mpris`, `network`, `notifications`. Built-in rail widgets:
`quick-settings` (decorative Misu mark), `workspaces` (5 fixed dots plus any
extra workspaces) and `tray`.

Each backend lives with the plugin that owns it. Other modules import it by
folder, e.g. `import "../network"`:

| plugin | service |
|---|---|
| audio | `Audio` (PipeWire) |
| battery | `Battery` (UPower), `PowerProfiles` (powerprofilesctl) |
| bluetooth | `BtLink` (BlueZ, bluetoothctl) |
| calendar | `TimeFormat` |
| network | `Network` (nmcli) |
| notifications | `Notifs` (notification server; also feeds the popups) |

Icons are Material Symbols ligature names (`"volume_up"`). Each module picks
its own names inline through `MaterialIcon { name: ...; size: ... }`.

## IPC

```sh
qs -c misu ipc call menu toggle audio       # any card: audio network bluetooth
                                            #   battery calendar notifications mpris aiusage
qs -c misu ipc call bar toggle              # hide/reveal the rail
qs -c misu ipc call osd brightness          # refresh brightness OSD after brightnessctl
```

## Config

Optional `~/.config/misu/config.json` (live-reloaded). Every key has a default:

| key | default | |
|---|---|---|
| `rail.size` | `48` | rail width (px) |
| `rail.reveal` | `true` | `false` hides the rail until hover |
| `rail.top` | `["quick-settings","workspaces"]` | widget ids per zone |
| `rail.center` | `["clock","ai-usage"]` | |
| `rail.bottom` | `["tray","notifications","audio-output","bluetooth","network","battery"]` | |
| `fontScale` | `1.3` | clamped 0.7–1.6 |
| `fontFamily` | `"Space Grotesk"` | |
| `frameThickness` / `frameCorner` / `frameOpacity` | `2` / `8` / `1` | |
| `markText` / `markImage` | `"力"` / `""` | rail brand mark |
| `clock24h` | `true` | |
| `reduceMotion` | `false` | |
| `cardMaxHeight` | `0.7` | tallest a card grows, as a fraction of the screen (0.3–0.95); taller content scrolls |

`rail` merges key by key over the default, so `{"rail":{"size":44}}` keeps the zones.

## Colors

`Theme` has a built-in palette and watches
`$XDG_STATE_HOME/rice/misu-colors.json` (default `~/.local/state/rice/`).
To generate that file, use the matugen template in the rice repo, `modules/theme/matugen/templates/misu-colors.json`
(it uses `{{colors.<role>.default.hex}}`) with that output path.
Rewriting the file recolors Misu live.

## Runtime dependencies

Binaries: `scrollmsg`, `nmcli`, `pactl`, `bluetoothctl`, `powerprofilesctl`,
`brightnessctl`, `sh`, and `ai-usagebar` (AUR `ai-usagebar-bin`; the AI usage
card runs `ai-usagebar usage --json` every five minutes and when opened. Claude
and Codex work from their CLIs' logins; other providers go in
`~/.config/ai-usagebar/config.toml`).

Services over D-Bus: PipeWire, UPower, BlueZ, NetworkManager, power-profiles-daemon.

Fonts: Material Symbols Rounded (icon glyphs), JetBrainsMono Nerd Font,
Noto Sans, Noto Sans CJK JP.

Quickshell must be built with Qt Multimedia Effects (`QtQuick.Effects`);
Qt5Compat is not needed.

Misu is the notification daemon, so don't run mako or dunst next to it.
The wallpaper is not misu's job; use swaybg.

## Testing

The former Nix container smoke test was removed with the Nix configuration.
Run Misu from a Quickshell session with `qs -c misu`.
