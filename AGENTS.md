# Agent notes

Misu, an Arch Linux rice: scroll (sway-scroll) compositor, Quickshell bar, matugen
palette. The checkout lives at `/opt/misu` (`install.sh` refuses to run from
elsewhere; `--dry-run` only warns) and `install.sh` symlinks config from it into
`$HOME`, so edits under `modules/` are live wherever it is installed.

## Layout

Everything installed belongs to a module in `modules/<name>/`, which has its
`packages.txt`, an `install.sh` (installs those packages, links its user config,
copies its root-owned files to `/etc`, `/usr/share`, `/opt`, enables its
services) and an `uninstall.sh` that undoes it. Module scripts are sourced by the
top-level `install.sh`, which holds the helpers (`run`, `as_user`, `link`,
`unlink_ours`, `install_packages`, `remove_packages`), bootstraps yay and runs
the modules in order; a module script run directly delegates to it.

```sh
sudo ./install.sh                        # every module
sudo ./install.sh bar theme              # just these
sudo modules/bar/install.sh              # same as ./install.sh bar
sudo ./uninstall.sh [--packages] [mod]   # --packages also removes their packages
./install.sh --dry-run [mod]             # prints every operation, no root needed
```

- `base`: build tools, yay, jq, network, Bluetooth, power, PipeWire, Quickshell,
  fonts, XDG user folders (`xdg-user-dirs`).
  Its uninstall keeps everything.
- `intel`: Intel graphics drivers (mesa, Vulkan, VA-API) and firmware
- `boot`: Limine as the boot loader (the old one stays on the ESP as a
  fallback). Boots straight in; a key press in the first second shows the menu.
  Entries per kernel and btrfs snapshot come from `limine-mkinitcpio-hook` and
  `limine-snapper-sync`; Misu owns only the globals at the top of `limine.conf`.
  Snapper's `root` config is written directly (snap-pac snapshots only, newest
  10 kept) around archinstall's `@.snapshots`.
- `greeter`: a Quickshell login screen (username and password) under Cage on
  greetd. Uses `bar`'s Space Grotesk font and `theme`'s wallpaper; it starts
  `wm/bin/misu-session` straight from the checkout.
- `wm`: scroll config, fuzzel, the session entry, and the `misu-*` commands
  in `bin/` (linked into `~/.local/bin`)
- `bar`: the Quickshell bar (config name `misu`). `plugins/aiusage` polls the
  `ai-usagebar` CLI for Claude and Codex usage.
- `theme`: matugen config and templates, wallpapers, GTK/Qt/cursor settings
- `shell`: zsh, kitty, starship
- `apps`: herdr, mimeapps, Zen defaults (all profiles), VS Code defaults, AI
  CLIs

When you add a package or an installed file, put it in its module's
`packages.txt` or `install.sh`, undo it in `uninstall.sh`, and check both with
`--dry-run`.

## Skills

`.agents/skills/` holds the workflows: `add-module`, `add-bar-plugin`,
`test-in-vm`.

## Rules

- Don't run `sudo ./install.sh` or `./uninstall.sh` on the host. The host is the user's daily machine.
  Test in the VM.
- Never commit or push until the user has verified the change and asked for it,
  every time. Earlier permission does not carry over to the next change.
- Keep every `install.sh` idempotent; they get re-run after changes.

## Testing in the KVM VM

`test/vm.sh` boots an Arch VM under KVM with virtio-gpu GL (needed for the
Wayland compositor), in an SDL window with app id `rice-vm` at the host's
resolution. Full screen comes from the host compositor: give app id `rice-vm`
a full-screen window rule (in niri, `window-rule { match app-id="^rice-vm$";
open-fullscreen true; }` in its config). Ctrl+Alt+G grabs keyboard and
pointer so Super shortcuts reach the guest; again to release.
State lives in a VM dir, `~/.cache/rice-vm` by default (`RICE_VM_DIR`
overrides): `arch.qcow2` (40G), UEFI vars, the ISO, and `qmp.sock`. Host needs `qemu-desktop`, `edk2-ovmf` and `socat`,
plus a writable `/dev/kvm`.

```sh
test/vm.sh                     # boot the disk; first run downloads the ISO and boots it
test/vm.sh --iso               # boot the ISO again (reinstall or rescue)
test/vm.sh -display none       # extra args go to qemu; this runs it headless
RICE_VM_DIR=/tmp/x test/vm.sh  # throwaway VM
```

It blocks until the VM powers off. Start it detached,
`setsid -f test/vm.sh >/dev/null 2>&1 </dev/null`, so the VM outlives an agent
session; a plain background job is killed with the session. Run long jobs in
the guest as `systemd-run` units for the same reason. Start over with
deleting `arch.qcow2` in the VM dir.

### First install (the user does this in the window)

In the ISO, run `archinstall` with: minimal profile, btrfs with subvolumes,
systemd-boot, NetworkManager, a regular user in `wheel` (any name), the user's
timezone, extra package `openssh`. After the reboot, as that user, mount the
host checkout at `/opt/misu` and install from it:

```sh
sudo systemctl enable --now sshd
sudo mkdir -p /opt/misu
echo 'rice /opt/misu 9p trans=virtio,version=9p2000.L,nofail 0 0' | sudo tee -a /etc/fstab
sudo systemctl daemon-reload && sudo mount /opt/misu
cd /opt/misu && sudo ./install.sh
```

### Driving the guest from the host

SSH goes to `127.0.0.1:2222` as the guest user. If sshd isn't up yet, ask the
user to log in. For password-less access, create a key pair on the host
(`ssh-keygen -t ed25519 -f <keyfile>`), install it once with
`ssh-copy-id -i <keyfile> -p 2222 <user>@127.0.0.1`, and pass `-i <keyfile>`
afterwards. Keep it somewhere that survives reboots: the VM dir keeps it with
the VM it belongs to, `~/.ssh` works too. A `Host` entry in `~/.ssh/config`
(HostName 127.0.0.1, Port 2222, User, IdentityFile) shortens it to `ssh <alias>`.

```sh
ssh -p 2222 <user>@127.0.0.1
# commands inside the running session need its environment; autostart.conf
# exports it to the systemd user manager, so import it from there:
ssh -p 2222 <user>@127.0.0.1 'export $(systemctl --user show-environment | grep -E "^(WAYLAND_DISPLAY|SWAYSOCK|SCROLLSOCK)="); scrollmsg reload'
```

Screenshot of the VM's display over QMP. It needs a non-GL display: the default
`sdl,gl=on` window answers "no surface", so start the VM with
`test/vm.sh -display egl-headless` for this, or use `grim` in the guest over SSH:

```sh
printf '%s\n' '{"execute":"qmp_capabilities"}' \
  '{"execute":"screendump","arguments":{"filename":"/tmp/vm.png","format":"png"}}' \
  | socat -t2 - UNIX-CONNECT:"${RICE_VM_DIR:-$HOME/.cache/rice-vm}/qmp.sock"
```

Then read `/tmp/vm.png`. Key input works through the same socket
(`{"execute":"send-key","arguments":{"keys":[{"type":"qcode","data":"ret"}]}}`).
