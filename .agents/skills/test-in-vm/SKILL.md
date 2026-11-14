---
name: test-in-vm
description: Boot the Misu KVM test VM, run install.sh or uninstall.sh in it, and inspect the session over SSH. Use to verify any change to modules, packages, or install scripts before it reaches the host.
---

# Test in the VM

Never run `sudo ./install.sh` or `./uninstall.sh` on the host. The VM mounts
this checkout over 9p (tag `rice`) at `/opt/misu`, so host edits are live in it.
`AGENTS.md` (Testing in the KVM VM) has the full setup and first install.

## Boot

```sh
pgrep -f qemu-system || setsid -f test/vm.sh >/dev/null 2>&1 </dev/null
```

Detached so it outlives the agent session. Wait for SSH on `127.0.0.1:2222`.

## SSH

Log in as the user created in archinstall (`<user>` below) on port 2222. Use a
key so commands don't prompt:

1. Look for one already set up: a `Host` entry for port 2222 in `~/.ssh/config`,
   or a key in the VM dir (`RICE_VM_DIR`, default `~/.cache/rice-vm`) or
   `~/.ssh`. Ask the user if unsure.
2. If there is none, create one somewhere that survives reboots (the VM dir
   keeps it with the VM): `ssh-keygen -t ed25519 -N '' -f <keyfile>`.
3. Authorize it once with `ssh-copy-id -i <keyfile> -p 2222 <user>@127.0.0.1`.
   That asks for the guest password; ask the user for it, or have them run it
   (`! ssh-copy-id ...`). Non-interactively it can come from `SSH_ASKPASS` with
   `SSH_ASKPASS_REQUIRE=force`.

```sh
ssh -p 2222 -i <keyfile> \
  -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null <user>@127.0.0.1
```

The host key changes when the VM is reinstalled, hence the two `-o` options.
`sudo` over SSH needs the password unless the VM gives the user passwordless
sudo (`echo '<user> ALL=(ALL) NOPASSWD: ALL' | sudo tee /etc/sudoers.d/vm`, in
the guest only).

## Guest-only settings

These fix QEMU, not the rice, so they go in the guest's system config and never
in a module:

- Cursor: wm draws each seat's cursor in the scene. `test/vm.sh` uses an SDL
  window with `show-cursor=off` to hide the host pointer over the guest.
- Shortcuts: Ctrl+Alt+G in the VM window grabs the keyboard, and the host
  (niri) then passes Super to the guest. That needs QEMU's SDL on Wayland,
  which `test/vm.sh` sets; under XWayland the host keeps Super.
- Scrolling: QEMU's tablet is a pointer, so touchpad settings do not apply.
  For guest-only natural scrolling, set `input.mouse.natural_scroll` in
  `~/.config/wm/config.json`, regenerate with `misu-wm-config` and reload wm.
- Builds that run out of memory (Electron apps from the AUR): add zram swap with
  `zram-generator` and `/etc/systemd/zram-generator.conf` (`[zram0]`,
  `zram-size = ram`).

## Run the installer

Long jobs run as a systemd unit so they survive a dropped SSH connection:

```sh
sudo systemctl reset-failed rice-install 2>/dev/null
sudo systemd-run --unit rice-install --setenv=SUDO_USER=<user> bash -c \
  'cd /opt/misu; ./install.sh [module...] >/var/log/rice-install.log 2>&1; echo EXIT=$? >>/var/log/rice-install.log'
```

`SUDO_USER` tells the installer whose home to set
up. Follow it with `sudo grep -E '^(== |EXIT=|error)' /var/log/rice-install.log`
and wait for `EXIT=`. Don't edit the top-level `install.sh` while it runs (bash
reads it as it goes). If pacman's lock is stale: `sudo rm /var/lib/pacman/db.lck`.

## Inspect the session

Reboot after the first full install to get the greeter. Commands inside the
running session import its environment:

```sh
export $(systemctl --user show-environment | grep -E '^(WAYLAND_DISPLAY|WM_SOCK|DISPLAY)=')
grim /tmp/s.png   # then scp it back (scp -P 2222) and read it
```

## Existing headless guest

Check `~/.cache/rice-vm/README.md` for existing SSH and image setup. When the
working checkout is in `~/Projects/misu`, run **that checkout's** `test/vm.sh`,
not the older host `/opt/misu/test/vm.sh`: the script shares its own parent
directory as the guest's `/opt/misu`. Keep the host install path check intact.
A new disk in another `RICE_VM_DIR` preserves the existing test guest. Never
remove a VM disk to get a fresh install when an isolated disk can be used.
Use `-display egl-headless` for GUI-free validation and `wm msg seat N screenshot`
or grim inside the guest for screenshots. Import `WM_SOCK` from the user manager.

For unattended AUR builds, enable lingering for the guest user. If repeated SSH
logouts remove fakeroot's SysV IPC objects (`semop: Invalid argument`), set
`[Login] RemoveIPC=no` in a guest-only logind drop-in and restart logind before
retrying. Keep this out of Misu's host modules. If a release signing key cannot
be fetched, provision the exact fingerprint declared by the package, then retry
with signature checking enabled.

For wm portal integration, import the session environment, then run
`MISU_PORTAL_UI=1 python /opt/misu/test/portal-check.py`. It builds a small
PipeWire consumer with `cc` and `pkg-config` (libpipewire and libpng headers),
confirms the chooser with Enter and writes a nonuniform captured screen image
to `/tmp/misu-portal-frame.png`. Run it only in the test desktop. With a changing
window, `MISU_PORTAL_FRAMES=3` checks successive frames.

Cloud images may automount the EFI partition. Limine's shell tools misread a
stacked autofs/vfat mount; give the guest an explicit vfat entry in fstab and
set its ESP_PATH in /etc/default/limine before the first boot-module install.
