# greeter: Misu's own Quickshell login screen (shell.qml) under Cage on greetd.
# The package's config.toml is kept as config.toml.pre-misu for uninstall.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

install_packages greeter
for p in greetd-regreet greetd-tuigreet; do if installed "$p"; then as_user yay -Rns --noconfirm "$p"; fi; done
run rm -f /etc/greetd/regreet.toml /etc/greetd/regreet.css

if [[ -f /etc/greetd/config.toml && ! -e /etc/greetd/config.toml.pre-misu ]] && ! cmp -s "$M/greeter/config.toml" /etc/greetd/config.toml; then
  run cp /etc/greetd/config.toml /etc/greetd/config.toml.pre-misu
fi
run install -Dm644 "$M/greeter/config.toml" /etc/greetd/config.toml

echo "+ install $M/greeter/shell.qml -> /etc/greetd/misu/shell.qml (@USER@=$target_user)"
(( DRY_RUN )) || sed -e "s|@USER@|$target_user|g" -e "s|@SESSION@|$M/wm/bin/misu-session|g" "$M/greeter/shell.qml" | install -Dm644 /dev/stdin /etc/greetd/misu/shell.qml

# Cage loads the "default" cursor theme, not XCURSOR_THEME; config.toml points
# XCURSOR_PATH at this one, which inherits the Misu cursor.
run install -Dm644 "$M/greeter/cursor/index.theme" /etc/greetd/misu/icons/default/index.theme

run systemctl enable greetd
