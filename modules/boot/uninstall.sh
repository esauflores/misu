# boot: puts back the limine.conf from before Misu, if there was one, and stops
# the snapshot entries. Limine stays installed and in the boot order (removing a
# boot loader is for the firmware menu or efibootmgr, by hand), and the Snapper
# config and its snapshots stay: they're the way back from a bad update.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

run systemctl disable --now limine-snapper-sync.service || true
esp=$(bootctl --print-esp-path 2>/dev/null || echo /boot)
if [[ -f $esp/limine.conf.pre-misu ]]; then
  run mv "$esp/limine.conf.pre-misu" "$esp/limine.conf"
  run limine-update
fi
