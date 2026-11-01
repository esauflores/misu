---
name: add-module
description: Add a Misu module (a self-contained piece of the rice with its own packages, install and uninstall), or add a package or installed file to an existing one. Use when adding software, config, or system files to the rice.
---

# Add a module

A module is `modules/<name>/` with three files plus its config:

```
modules/<name>/
  packages.txt    # one package per line, # comments; official repo and AUR alike
  install.sh      # installs packages, links config, copies root-owned files
  uninstall.sh    # undoes install.sh
```

The top-level `install.sh` defines the helpers and sources each module's script
in the order of its `MODULES=(...)` array; `uninstall.sh` runs them in reverse.
Module scripts are fragments, not standalone programs: no shebang, no
`set -euo pipefail` (the caller has it), and their first line delegates to the
top-level script when run directly.

## Steps

1. Pick the module the change belongs to. Only make a new one for a separate
   concern someone could install or remove alone (e.g. `intel` for hardware).
2. Create the folder and `packages.txt`.
3. Write `install.sh`, copying the header shape of an existing module:

   ```bash
   # <name>: what it sets up, in one or two lines.
   [[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

   install_packages <name>

   link "$M/<name>/foo.conf" "$home/.config/foo/foo.conf"
   run install -Dm644 "$M/<name>/bar.conf" /etc/bar.conf
   run systemctl enable bar
   ```

4. Write `uninstall.sh` (same guard with `--uninstall` before the module name):

   ```bash
   # <name>: what it removes and what it deliberately leaves.
   [[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

   run systemctl disable bar
   run rm -f /etc/bar.conf
   unlink_ours "$home/.config/foo/foo.conf"

   remove_packages <name>
   ```

5. `chmod +x` both scripts and add the name to `MODULES` in the top-level
   `install.sh`, after the modules it depends on.
6. List it in `AGENTS.md` (Layout) and `README.md` (Layout).
7. Check: `./install.sh --dry-run <name>` and
   `./uninstall.sh --dry-run --packages <name>`, then `bash -n` on both scripts.
   Test for real in the VM (see the `test-in-vm` skill), never on the host.

## Helpers

| helper | does |
|---|---|
| `run cmd...` | runs as root, printed; skipped under `--dry-run` |
| `as_user cmd...` | runs as the target user (`$target_user`) |
| `user_bus cmd...` | as the user with their session bus, for `gsettings`, `systemctl --user` |
| `link src dest` | symlinks `dest` to `src` in the checkout, making the parent |
| `unlink_ours dest` | removes `dest` only if it links into this checkout |
| `installed pkg` | true if the package is installed |
| `install_packages <name>` | `yay -S --needed` the module's `packages.txt` |
| `remove_packages <name>` | with `--packages`, `pacman -Rnsu` them (skips ones still needed) |

Variables: `$ROOT` (checkout), `$M` (`$ROOT/modules`), `$home`, `$target_user`,
`$user_uid`, `$DRY_RUN`.

## Rules

- Idempotent: re-running must be safe. `install -D`, `ln -sfn`, `--needed`,
  and guard one-time steps (see how `greeter` backs up `config.toml` once).
- Anything that writes outside `run`/`as_user`/`link` must check `$DRY_RUN`
  itself (see the `wm` desktop entry and the `apps` VS Code merge).
- Prefer linking user config into the checkout so edits are live; copy only
  root-owned files (`/etc`, `/usr/share`, `/opt`).
- Uninstall never touches user data and only removes links it owns.
- Packages live in exactly one module. A package several modules need goes in
  `base`.
