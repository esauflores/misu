# shell: removes the links and switches the login shell back to bash.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

unlink_ours "$home/.zshrc"
unlink_ours "$home/.config/foot/foot.ini"
# Remove an untouched shell-only fallback; leave generated wallpaper palettes.
if cmp -s "$M/shell/foot-colors.ini" "$home/.local/state/rice/foot-colors.ini"; then
  as_user rm "$home/.local/state/rice/foot-colors.ini"
fi
unlink_ours "$home/.config/starship.toml"
run chsh -s /bin/bash "$target_user"

remove_packages shell
