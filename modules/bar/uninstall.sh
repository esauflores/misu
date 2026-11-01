# bar: removes the Quickshell config link.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

unlink_ours "$home/.config/quickshell/misu"
remove_packages bar
