# dictate: stops the transcriber, removes the commands, the key binding, the
# downloaded models and the settings. Saved dictations (~/Documents/Dictations)
# stay.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

user_bus systemctl --user stop misu-dictate-rec misu-dictate-server 2>/dev/null || true
for f in "$M"/dictate/bin/*; do unlink_ours "$home/.local/bin/${f##*/}"; done
# Remove a binding left by the previous compositor installation.
run rm -f /etc/scroll/config.d/misu-dictate.conf
as_user rm -rf "$home/.cache/misu-dictate" "/run/user/$user_uid/misu-dictate" \
  "$home/.local/state/rice/dictate-source" "$home/.local/state/rice/dictate-lang"

remove_packages dictate
