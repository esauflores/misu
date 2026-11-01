# wm: scroll, fuzzel, the misu-* commands, and the wayland-sessions entry.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

install_packages wm

# The session starts from the checkout, so it works for every user.
echo "+ install $M/wm/misu.desktop -> /usr/share/wayland-sessions/misu.desktop (@SESSION@=$M/wm/bin/misu-session)"
(( DRY_RUN )) || sed "s|@SESSION@|$M/wm/bin/misu-session|g" "$M/wm/misu.desktop" | install -Dm644 /dev/stdin /usr/share/wayland-sessions/misu.desktop

link "$M/wm/scroll" "$home/.config/scroll"
link "$M/wm/fuzzel.ini" "$home/.config/fuzzel/fuzzel.ini"
for f in "$M"/wm/bin/*; do link "$f" "$home/.local/bin/${f##*/}"; done
