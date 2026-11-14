# base: does nothing. Its packages and services carry the network and
# audio, so they stay even with --packages, as does NetworkManager's iwd backend
# (/etc/NetworkManager/conf.d/misu-iwd.conf) and BlueZ's AlwaysPairable
# (/etc/bluetooth/main.conf); remove them by hand if you mean it.
# The XDG user folders are user data and stay too.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

:
