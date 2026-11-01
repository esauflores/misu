# Agent notes

Misu, an Arch Linux rice: scroll (sway-scroll) compositor, Quickshell bar, matugen
palette. `install.sh` symlinks config from this checkout into `$HOME`, so edits
under `modules/` and `bin/` are live wherever it is installed.

## Layout

Everything installed belongs to a module in `modules/<name>/`. `install.sh`
installs `packages.txt` with `yay`, enables services, links `bin/*` into
`~/.local/bin`, then has one section per module that links its user config and
copies its root-owned files (to `/etc`, `/usr/share`).

- `wm`: scroll config, fuzzel, the greeter session entry
- `bar`: the Quickshell bar (config name `misu`). `plugins/aiusage` polls the
  `ai-usagebar` CLI for every AI plan's usage.
- `theme`: matugen config and templates, wallpapers, GTK/Qt/cursor settings
- `greeter`: Regreet under Cage on greetd
- `shell`: zsh, kitty, starship, git identity
- `apps`: herdr, mimeapps, Zen defaults (all profiles), VS Code defaults

When you add an installed file, add it to its module's section in `install.sh`
and check with `./install.sh --dry-run` (no root needed; it prints every
operation and changes nothing).

## Rules

- Don't run `sudo ./install.sh` on the host. The host is the user's daily machine.
  Test in the VM.
- Don't commit or push unless asked.
- Keep `install.sh` idempotent; it gets re-run after changes.

## Testing in the KVM VM

`test/vm.sh` boots an Arch VM under KVM with virtio-gpu GL (needed for the
Wayland compositor), in a GTK window on the host. State lives in
`~/.cache/rice-vm/` (`RICE_VM_DIR` overrides): `arch.qcow2` (40G), UEFI vars,
the ISO, and `qmp.sock`. Host needs `qemu-desktop`, `edk2-ovmf` and `socat`,
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
`rm ~/.cache/rice-vm/arch.qcow2`.

### First install (the user does this in the window)

In the ISO, run `archinstall` with: minimal profile, btrfs with subvolumes,
systemd-boot, NetworkManager, user `esauflores` (uid 1000, wheel), timezone
America/El_Salvador, extra package `openssh`. After the reboot, as that user:

```sh
sudo systemctl enable --now sshd
mkdir -p ~/Projects/rice
echo 'rice /home/esauflores/Projects/rice 9p trans=virtio,version=9p2000.L,nofail 0 0' | sudo tee -a /etc/fstab
sudo systemctl daemon-reload && sudo mount ~/Projects/rice
cd ~/Projects/rice && sudo ./install.sh
```

The repo is the host checkout shared over 9p (tag `rice`), not a clone, so
host edits show up in the guest right away and the guest's links point at them.

### Driving the guest from the host

SSH goes to `127.0.0.1:2222`. If sshd isn't up yet, ask the user to log in.

```sh
ssh -p 2222 esauflores@127.0.0.1
# commands inside the running session need its environment; autostart.conf
# exports it to the systemd user manager, so import it from there:
ssh -p 2222 esauflores@127.0.0.1 'export $(systemctl --user show-environment | grep -E "^(WAYLAND_DISPLAY|SWAYSOCK|SCROLLSOCK)="); scrollmsg reload'
```

Screenshot of the VM's display over QMP. It needs a non-GL display: the default
`gtk,gl=on` window answers "no surface", so start the VM with
`test/vm.sh -display egl-headless` for this, or use `grim` in the guest over SSH:

```sh
printf '%s\n' '{"execute":"qmp_capabilities"}' \
  '{"execute":"screendump","arguments":{"filename":"/tmp/vm.png","format":"png"}}' \
  | socat -t2 - UNIX-CONNECT:$HOME/.cache/rice-vm/qmp.sock
```

Then read `/tmp/vm.png`. Key input works through the same socket
(`{"execute":"send-key","arguments":{"keys":[{"type":"qcode","data":"ret"}]}}`).

If the cursor doesn't show in the guest session, set
`WLR_NO_HARDWARE_CURSORS=1` there.
