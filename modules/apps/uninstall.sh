# apps: removes the links and the Zen defaults, and drops from VS Code's settings
# each Misu default that still has Misu's value.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

unlink_ours "$home/.config/herdr/config.toml"
unlink_ours "$home/.config/mimeapps.list"
run rm -f /opt/zen-browser-bin/browser/defaults/preferences/misu.js

vscode_settings="$home/.config/Code/User/settings.json"
if (( DRY_RUN )); then
  echo "+ remove Misu defaults from $vscode_settings"
elif as_user jq -e . "$vscode_settings" >/dev/null 2>&1; then
  as_user bash -c 'tmp=$(mktemp); jq --slurpfile m "$2" "reduce (\$m[0] | keys[]) as \$k (.; if .[\$k] == \$m[0][\$k] then del(.[\$k]) else . end)" "$1" >"$tmp" && cat "$tmp" >"$1"; status=$?; rm -f "$tmp"; exit "$status"' _ "$vscode_settings" "$M/apps/vscode/settings.json"
fi

remove_packages apps
