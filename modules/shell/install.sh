# shell: zsh as login shell, kitty, starship.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

install_packages shell

link "$M/shell/zshrc" "$home/.zshrc"
link "$M/shell/kitty.conf" "$home/.config/kitty/kitty.conf"
link "$M/shell/starship.toml" "$home/.config/starship.toml"

run chsh -s /bin/zsh "$target_user"
