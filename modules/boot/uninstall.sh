# boot: puts back the limine.conf from before Misu, if there was one, and stops
# the snapshot entries. Limine stays installed and in the boot order (removing a
# boot loader is for the firmware menu or efibootmgr, by hand), and the Snapper
# config and its snapshots stay: they're the way back from a bad update.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

run systemctl disable --now limine-snapper-sync.service || true
# The quiet boot's words leave the kernel command line (the next kernel
# update writes it into the entries).
for word in quiet loglevel=3 systemd.show_status=auto rd.udev.log_level=3; do
  if grep -qw -- "$word" /etc/kernel/cmdline 2>/dev/null; then
    run sed -i "1s/ $word\b//" /etc/kernel/cmdline
  fi
done
esp=$(bootctl --print-esp-path 2>/dev/null || echo /boot)
if [[ -f $esp/limine.conf.pre-misu ]]; then
  run mv "$esp/limine.conf.pre-misu" "$esp/limine.conf"
  run limine-update
fi
