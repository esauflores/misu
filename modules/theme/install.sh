# theme: matugen palette, GTK/Qt/cursor settings, default wallpaper (also used by
# the greeter and misu-lock).
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

install_packages theme

link "$M/theme/matugen" "$home/.config/matugen"
link "$M/theme/gtk-3.0/settings.ini" "$home/.config/gtk-3.0/settings.ini"
link "$M/theme/gtk-4.0/settings.ini" "$home/.config/gtk-4.0/settings.ini"
link "$M/theme/qt6ct/qt6ct.conf" "$home/.config/qt6ct/qt6ct.conf"

run install -Dm644 "$M/theme/wallpapers/black-hole.png" /usr/share/backgrounds/rice/black-hole.png
# Older installs had it as WebP.
run rm -f /usr/share/backgrounds/rice/black-hole.webp
if [[ ! -e $home/Pictures/Wallpapers/black-hole.png ]]; then
  as_user install -Dm644 "$M/theme/wallpapers/black-hole.png" "$home/Pictures/Wallpapers/black-hole.png"
fi

while read -r key value; do user_bus gsettings set org.gnome.desktop.interface "$key" "$value"; done <<'KEYS'
color-scheme prefer-dark
gtk-theme adw-gtk3-dark
icon-theme Adwaita
cursor-theme Bibata-Modern-Ice
cursor-size 20
KEYS

as_user "$M/wm/bin/misu-wall" restore
