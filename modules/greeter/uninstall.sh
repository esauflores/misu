# greeter: disables greetd, restores its original config and drops the
# gnome-keyring lines from its PAM file.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

run systemctl disable greetd || true
run rm -rf /etc/greetd/misu
if [[ -e /etc/greetd/config.toml.pre-misu ]]; then run mv /etc/greetd/config.toml.pre-misu /etc/greetd/config.toml; fi
if grep -qx '# misu: gnome-keyring' /etc/pam.d/greetd 2>/dev/null; then
  run sed -i '/^# misu: gnome-keyring$/,+2d' /etc/pam.d/greetd
fi

remove_packages greeter
