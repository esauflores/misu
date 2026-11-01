#!/usr/bin/env bash
# Install Misu: packages, root-owned files, services, and symlinks from $HOME into
# this checkout. Idempotent. --dry-run prints every operation and changes nothing.
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
M=$ROOT/modules

DRY_RUN=0
[[ ${1:-} == --dry-run ]] && DRY_RUN=1
if (( ! DRY_RUN && EUID != 0 )); then echo 'Run via sudo: sudo ./install.sh' >&2; exit 1; fi

target_user=${SUDO_USER:-$USER}
home=$(getent passwd "$target_user" | cut -d: -f6)
user_uid=$(id -u "$target_user")

run() { printf '+ '; printf '%q ' "$@"; printf '\n'; (( DRY_RUN )) || "$@"; }
as_user() { if (( DRY_RUN )); then printf '+ sudo -u %q ' "$target_user"; printf '%q ' "$@"; printf '\n'; else sudo -u "$target_user" "$@"; fi; }
user_bus() { as_user env XDG_RUNTIME_DIR="/run/user/$user_uid" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$user_uid/bus" "$@"; }
link() { as_user install -d "$(dirname "$2")"; as_user ln -sfn "$1" "$2"; }

# Packages
if ! command -v yay >/dev/null; then
  if (( DRY_RUN )); then echo '+ bootstrap yay from the AUR';
  else
    run pacman -S --needed --noconfirm base-devel git
    as_user git clone https://aur.archlinux.org/yay.git "$home/yay"
    as_user bash -lc "cd '$home/yay' && makepkg -si --noconfirm"
    run rm -rf "$home/yay"
  fi
fi

mapfile -t packages < <(grep -Ev '^[[:space:]]*(#|$)' "$ROOT/packages.txt")

# The repo's `code` conflicts with visual-studio-code-bin, tuigreet is replaced by Regreet.
if pacman -Qq code >/dev/null 2>&1; then as_user yay -Rns --noconfirm code; fi
as_user yay -Syu --needed --noconfirm "${packages[@]}"
if pacman -Qq greetd-tuigreet >/dev/null 2>&1; then as_user yay -Rns --noconfirm greetd-tuigreet; fi

run systemctl enable NetworkManager bluetooth upower power-profiles-daemon greetd
for f in "$ROOT"/bin/*; do link "$f" "$home/.local/bin/${f##*/}"; done

# greeter: Regreet under Cage on greetd
for f in config.toml regreet.toml regreet.css; do run install -Dm644 "$M/greeter/$f" "/etc/greetd/$f"; done

# wm: scroll, fuzzel, and the session entry the greeter lists
echo "+ install $M/wm/misu.desktop -> /usr/share/wayland-sessions/misu.desktop (@HOME@=$home)"
(( DRY_RUN )) || sed "s|@HOME@|$home|g" "$M/wm/misu.desktop" | install -Dm644 /dev/stdin /usr/share/wayland-sessions/misu.desktop

link "$M/wm/scroll" "$home/.config/scroll"
link "$M/wm/fuzzel.ini" "$home/.config/fuzzel/fuzzel.ini"

# bar: Quickshell config
link "$M/bar" "$home/.config/quickshell/misu"

# theme: matugen palette, GTK/Qt/cursor settings, default wallpaper (also used by
# the greeter and misu-lock)
link "$M/theme/matugen" "$home/.config/matugen"
link "$M/theme/gtk-3.0/settings.ini" "$home/.config/gtk-3.0/settings.ini"
link "$M/theme/gtk-4.0/settings.ini" "$home/.config/gtk-4.0/settings.ini"
link "$M/theme/qt6ct/qt6ct.conf" "$home/.config/qt6ct/qt6ct.conf"

run install -Dm644 "$M/theme/wallpapers/black-hole.webp" /usr/share/backgrounds/rice/black-hole.webp
if [[ ! -e $home/Pictures/Wallpapers/black-hole.webp ]]; then
  as_user install -Dm644 "$M/theme/wallpapers/black-hole.webp" "$home/Pictures/Wallpapers/black-hole.webp"
fi

while read -r key value; do user_bus gsettings set org.gnome.desktop.interface "$key" "$value"; done <<'EOF'
color-scheme prefer-dark
gtk-theme adw-gtk3-dark
icon-theme Adwaita
cursor-theme Bibata-Modern-Ice
cursor-size 20
EOF

as_user "$home/.local/bin/misu-wall" restore

# shell: zsh as login shell, kitty, starship, git identity
link "$M/shell/zshrc" "$home/.zshrc"
link "$M/shell/kitty.conf" "$home/.config/kitty/kitty.conf"
link "$M/shell/starship.toml" "$home/.config/starship.toml"

as_user git config --global user.name 'Cesar Flores'
as_user git config --global user.email 'dev@esauflores.com'
run chsh -s /bin/zsh "$target_user"

# apps: herdr, default applications, VS Code defaults merged into existing settings
link "$M/apps/herdr" "$home/.config/herdr"
link "$M/apps/mimeapps.list" "$home/.config/mimeapps.list"

vscode_settings="$home/.config/Code/User/settings.json"
if (( DRY_RUN )); then
  echo "+ merge $M/apps/vscode/settings.json into $vscode_settings"
elif [[ -f $vscode_settings ]]; then
  if as_user jq -e . "$vscode_settings" >/dev/null 2>&1; then
    as_user bash -c 'tmp=$(mktemp); jq -s ".[0] * .[1]" "$1" "$2" >"$tmp" && cat "$tmp" >"$1"; status=$?; rm -f "$tmp"; exit "$status"' _ "$vscode_settings" "$M/apps/vscode/settings.json"
  else
    echo "Skipping VS Code defaults: $vscode_settings is not valid JSON" >&2
  fi
else
  as_user install -Dm644 "$M/apps/vscode/settings.json" "$vscode_settings"
fi

# User services
user_bus systemctl --user enable --now pipewire.socket pipewire-pulse.socket wireplumber.service

echo 'Install complete. Reboot to start the greetd session.'
