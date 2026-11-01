# theme: removes the config links and system wallpaper and resets the GTK
# settings. Wallpapers in ~/Pictures/Wallpapers stay.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

unlink_ours "$home/.config/matugen"
unlink_ours "$home/.config/gtk-3.0/settings.ini"
unlink_ours "$home/.config/gtk-4.0/settings.ini"
unlink_ours "$home/.config/qt6ct/qt6ct.conf"
run rm -rf /usr/share/backgrounds/rice

for key in color-scheme gtk-theme icon-theme cursor-theme cursor-size; do
  user_bus gsettings reset org.gnome.desktop.interface "$key"
done

remove_packages theme
