# greeter: disables greetd and restores its original config.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

run systemctl disable greetd || true
run rm -rf /etc/greetd/misu
if [[ -e /etc/greetd/config.toml.pre-misu ]]; then run mv /etc/greetd/config.toml.pre-misu /etc/greetd/config.toml; fi

remove_packages greeter
