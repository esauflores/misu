# bar: the Quickshell bar, config name misu.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

install_packages bar
link "$M/bar" "$home/.config/quickshell/misu"
