#!/usr/bin/env bash
# Install Misu from its checkout at /opt/misu. Each module in modules/<name>/ has its packages.txt, an install.sh
# that installs those packages and links its config from $HOME into this
# checkout, and an uninstall.sh that undoes it. This script holds the shared
# helpers and runs the modules in order; module scripts run alone delegate here.
#
#   sudo ./install.sh [--dry-run] [module...]              all modules by default
#   sudo ./install.sh --uninstall [--packages] [module...] same as ./uninstall.sh
#
# Idempotent. --dry-run prints every operation and changes nothing (no root needed).
# --packages also removes the modules' packages that nothing else needs.
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
M=$ROOT/modules
MODULES=(base intel boot greeter wm bar theme shell apps)

DRY_RUN=0 UNINSTALL=0 PURGE=0 selected=()
for arg; do
  case $arg in
    --dry-run) DRY_RUN=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --packages) PURGE=1 ;;
    -*) echo "unknown option: $arg" >&2; exit 1 ;;
    *) [[ -d $M/$arg ]] || { echo "unknown module: $arg" >&2; exit 1; }; selected+=("$arg") ;;
  esac
done
(( ${#selected[@]} )) || selected=("${MODULES[@]}")

if (( ! DRY_RUN && EUID != 0 )); then echo "Run via sudo: sudo $0 $*" >&2; exit 1; fi

target_user=${SUDO_USER:-$USER}
if [[ $target_user == root ]]; then echo "Run via sudo from your user account, not as root." >&2; exit 1; fi

# Every user's links point into the checkout, so it lives at one stable path
# (a symlink to it is fine; pwd -P resolves it).
PREFIX=/opt/misu
if [[ $ROOT != "$PREFIX" ]]; then
  msg="Misu belongs at $PREFIX, not $ROOT. Move it: sudo mv $ROOT $PREFIX && sudo chown -R $target_user: $PREFIX"
  if (( DRY_RUN )); then echo "warning: $msg" >&2; else echo "$msg" >&2; exit 1; fi
fi

home=$(getent passwd "$target_user" | cut -d: -f6)
user_uid=$(id -u "$target_user")

run() { printf '+ '; printf '%q ' "$@"; printf '\n'; (( DRY_RUN )) || "$@"; }
as_user() { if (( DRY_RUN )); then printf '+ sudo -u %q ' "$target_user"; printf '%q ' "$@"; printf '\n'; else sudo -u "$target_user" "$@"; fi; }
user_bus() { as_user env XDG_RUNTIME_DIR="/run/user/$user_uid" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$user_uid/bus" "$@"; }
link() { as_user install -d "$(dirname "$2")"; as_user ln -sfn "$1" "$2"; }

# Remove a link only if it points into this checkout, so user files are left alone.
unlink_ours() { if [[ -L $1 && $(readlink "$1") == "$ROOT"/* ]]; then as_user rm "$1"; fi; }

# Exact name only: pacman -Q also answers for a package that provides it
# (visual-studio-code-bin provides code).
installed() { [[ $(pacman -Qq "$1" 2>/dev/null) == "$1" ]]; }
module_packages() { grep -Ev '^[[:space:]]*(#|$)' "$M/$1/packages.txt" 2>/dev/null || true; }

# Install the calling module's packages.txt.
install_packages() {
  local pkgs; mapfile -t pkgs < <(module_packages "$1")
  if (( ${#pkgs[@]} )); then as_user yay -S --needed --noconfirm --sudoloop "${pkgs[@]}"; fi
}

# With --packages, remove the module's installed packages; -u skips any that
# another package still depends on.
remove_packages() {
  (( PURGE )) || return 0
  local p pkgs=()
  for p in $(module_packages "$1"); do installed "$p" && pkgs+=("$p"); done
  if (( ${#pkgs[@]} )); then run pacman -Rnsu --noconfirm "${pkgs[@]}"; fi
}

if (( UNINSTALL )); then
  for (( i = ${#MODULES[@]} - 1; i >= 0; i-- )); do
    m=${MODULES[i]}
    [[ " ${selected[*]} " == *" $m "* ]] || continue
    echo "== uninstall $m"
    source "$M/$m/uninstall.sh"
  done
  echo 'Uninstall complete.'
  exit 0
fi

# yay installs every module's packages, so bootstrap it and sync once up front.
if ! command -v yay >/dev/null; then
  if (( DRY_RUN )); then echo '+ bootstrap yay from the AUR';
  else
    run pacman -S --needed --noconfirm base-devel git
    run rm -rf "$home/yay"
    as_user git clone https://aur.archlinux.org/yay.git "$home/yay"
    as_user bash -lc "cd '$home/yay' && makepkg -si --noconfirm"
    run rm -rf "$home/yay"
  fi
fi

as_user yay -Syu --noconfirm --sudoloop

for m in "${MODULES[@]}"; do
  [[ " ${selected[*]} " == *" $m "* ]] || continue
  echo "== install $m"
  source "$M/$m/install.sh"
done

echo 'Install complete. Reboot to start the greetd session.'
