# shell: removes the links and switches the login shell back to bash.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

unlink_ours "$home/.zshrc"
unlink_ours "$home/.config/kitty/kitty.conf"
unlink_ours "$home/.config/starship.toml"
run chsh -s /bin/bash "$target_user"

remove_packages shell
