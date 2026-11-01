# wm: removes the session entry, the scroll and fuzzel links, and the misu-* commands.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

run rm -f /usr/share/wayland-sessions/misu.desktop
unlink_ours "$home/.config/scroll"
unlink_ours "$home/.config/fuzzel/fuzzel.ini"
for f in "$M"/wm/bin/*; do unlink_ours "$home/.local/bin/${f##*/}"; done

remove_packages wm
