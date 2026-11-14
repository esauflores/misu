# base: build tools, network, Bluetooth, power, audio, fonts, XDG user folders.
# Sourced by ../../install.sh, which defines the helpers; run alone, it delegates there.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

install_packages base

# NetworkManager owns the network, with iwd as its Wi-Fi backend (the bar talks
# to it over nmcli). archinstall can leave systemd-networkd configuring the same
# links; it would fight NetworkManager, so it goes. Applies from the next boot.
run install -Dm644 "$M/base/NetworkManager/misu-iwd.conf" /etc/NetworkManager/conf.d/misu-iwd.conf
run systemctl disable systemd-networkd.service systemd-networkd.socket systemd-networkd-wait-online.service
# BlueZ bonds (stores a device's keys) only while it's pairable, and without
# AlwaysPairable that's only while an agent is registered; Misu runs none, so
# devices paired but never bonded, and each connect had to pair again.
if (( DRY_RUN )); then
  echo "+ set AlwaysPairable = true in /etc/bluetooth/main.conf"
elif ! grep -qx 'AlwaysPairable = true' /etc/bluetooth/main.conf; then
  sed -i 's/^#\?AlwaysPairable *=.*/AlwaysPairable = true/' /etc/bluetooth/main.conf
fi

run systemctl enable NetworkManager iwd bluetooth upower power-profiles-daemon
user_bus systemctl --user enable --now pipewire.socket pipewire-pulse.socket wireplumber.service

# Desktop, Documents, Downloads, Music, Pictures, Public, Templates, Videos in
# ~/.config/user-dirs.dirs, created if missing; entries already set are kept.
as_user env HOME="$home" xdg-user-dirs-update
