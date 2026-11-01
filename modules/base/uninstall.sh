# base: does nothing. Its packages and services carry the network and
# audio, so they stay even with --packages; remove them by hand if you mean it.
# The XDG user folders are user data and stay too.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

:
