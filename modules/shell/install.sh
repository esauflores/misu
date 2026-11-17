# shell: zsh as login shell, foot, starship.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

install_packages shell

link "$M/shell/zshrc" "$home/.zshrc"
# foot requires its include to exist, including a shell-only install.
if [[ ! -f $home/.local/state/rice/foot-colors.ini ]]; then
  as_user install -Dm644 "$M/shell/foot-colors.ini" "$home/.local/state/rice/foot-colors.ini"
fi
link "$M/shell/foot.ini" "$home/.config/foot/foot.ini"
link "$M/shell/starship.toml" "$home/.config/starship.toml"

run chsh -s /bin/zsh "$target_user"
