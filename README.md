# Misu

Misu is an Arch Linux rice: the scroll desktop, a Quickshell bar, shell,
applications and matugen color palette. The config files link back to this
checkout, so edits apply immediately.

## Install

Run `sudo ./install.sh` on an Arch install. `sudo ./install.sh --dry-run` prints the
planned package and file operations. The installer installs the packages,
sets up the Regreet login screen under Cage, sets scroll system config, links
user config, and seeds the wallpaper palette. The lock screen follows the
selected wallpaper and uses Misu colors. It expects the account running sudo
to be `esauflores` and the repo
to be at `~/Projects/rice`.

For a clean install, use Archinstall with btrfs subvolumes, systemd-boot,
NetworkManager and user `esauflores`, then clone this repository to
`~/Projects/rice` and run the installer.

## Layout

- `modules/`: `wm` (scroll), `bar` (Quickshell), `theme` (palette, GTK/Qt),
  `greeter` (Regreet), `shell`, `apps`; each has its section in `install.sh`
- `bin/`: commands installed under `~/.local/bin`
- `packages.txt`: pacman and AUR packages
- `test/vm.sh`: Arch test VM under KVM

Test changes in the KVM VM before touching the laptop; see `AGENTS.md`. User
setup after installation includes signing into Zen, VS Code, Claude and Codex
and enabling Snapper timeline and cleanup.
