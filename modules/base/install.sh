# base: build tools, network, Bluetooth, power, audio, fonts, XDG user folders.
# Sourced by ../../install.sh, which defines the helpers; run alone, it delegates there.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

install_packages base

run systemctl enable NetworkManager bluetooth upower power-profiles-daemon
user_bus systemctl --user enable --now pipewire.socket pipewire-pulse.socket wireplumber.service

# Desktop, Documents, Downloads, Music, Pictures, Public, Templates, Videos in
# ~/.config/user-dirs.dirs, created if missing; entries already set are kept.
as_user env HOME="$home" xdg-user-dirs-update
