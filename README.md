# Misu

Misu is an Arch Linux rice: the scroll desktop, a Quickshell bar, shell,
applications and matugen color palette. The config files link back to this
checkout, so edits apply immediately.

## Install

Misu lives at `/opt/misu`, owned by the user who edits it, and every user's
config links point there. The installer refuses to run from anywhere else.
Clone it there and run the installer from each account that should get the
rice; it sets up the home of whoever ran sudo.

```sh
sudo pacman -S --needed git
sudo git clone <repo-url> /opt/misu && sudo chown -R "$USER": /opt/misu
cd /opt/misu
sudo ./install.sh                         # every module
sudo ./install.sh bar theme               # just these
./install.sh --dry-run                    # print the plan, change nothing
sudo ./uninstall.sh [--packages] [mod]    # undo; --packages also removes packages
```

The installer installs the packages, sets up Limine and the Quickshell login
screen under Cage, links user config, and seeds the wallpaper palette. The lock screen follows the selected wallpaper and uses Misu colors.

For a clean install, use Archinstall with btrfs subvolumes, systemd-boot (the
`boot` module switches to Limine and keeps systemd-boot as a fallback),
NetworkManager and a regular user in `wheel`, then log in as that user and
follow the steps above. To keep working on it from your projects folder, link
it: `ln -s /opt/misu ~/Projects/misu`.

## Layout

- `modules/`: `base` (network, audio, fonts), `intel` (drivers, firmware),
  `boot` (Limine, straight to the login screen; Snapper snapshots),
  `greeter` (login screen), `wm` (scroll and the `misu-*` commands), `bar`
  (Quickshell), `theme` (palette, GTK/Qt), `shell`, `apps`; each has its
  `packages.txt`, `install.sh` and `uninstall.sh`
- `test/vm.sh`: Arch test VM under KVM

Test changes in the KVM VM before touching the laptop; see `AGENTS.md`. User
setup after installation includes signing into Zen, VS Code, Claude and Codex
and setting the git identity.

Every package install or update takes a btrfs snapshot before and after (the
newest 10 are kept), and each shows up in the boot menu: press a key at boot,
pick the one from before a bad update, then `sudo limine-snapper-restore` to
make it the system again.
