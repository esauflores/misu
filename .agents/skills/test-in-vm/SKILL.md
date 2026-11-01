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

- Cursor: QEMU shows the guest's hardware cursor inverted, so the guest draws
  its own: `echo WLR_NO_HARDWARE_CURSORS=1 | sudo tee -a /etc/environment`, then
  log out and back in. `test/vm.sh` uses an SDL window (`show-cursor=off`), which
  hides the host pointer; GTK showed it as a second cursor.
- Shortcuts: Ctrl+Alt+G in the VM window grabs the keyboard, and the host
  (niri) then passes Super to the guest. That needs QEMU's SDL on Wayland,
  which `test/vm.sh` sets; under XWayland the host keeps Super.
- Scrolling: the host touchpad reaches the guest as `QEMU_Virtio_Tablet`, a
  pointer, so the rice's touchpad `natural_scroll` misses it:
  `/etc/scroll/config.d/vm.conf` with
  `input "1575:3:QEMU_Virtio_Tablet" natural_scroll enabled`.
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
export $(systemctl --user show-environment | grep -E '^(WAYLAND_DISPLAY|SWAYSOCK|SCROLLSOCK)=')
grim /tmp/s.png   # then scp it back (scp -P 2222) and read it
```
