# boot: Limine as the boot loader, booting straight in (menu on a key press),
# with entries for every kernel and btrfs snapshot, and Snapper taking those
# snapshots around every package transaction. The previous boot loader stays on
# the ESP as a fallback.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

# The generated entries need a kernel command line, so write it before the
# packages' hook first runs. Take it once from the running boot, minus what the
# old boot loader added itself.
if [[ ! -s /etc/kernel/cmdline ]]; then
  cmdline=$(tr ' ' '\n' </proc/cmdline | grep -Ev '^(initrd|BOOT_IMAGE)=|^$' | paste -sd ' ')
  if (( DRY_RUN )); then echo "+ write /etc/kernel/cmdline: $cmdline"
  else install -d /etc/kernel; printf '%s\n' "$cmdline" >/etc/kernel/cmdline; fi
fi

esp=$(bootctl --print-esp-path 2>/dev/null || echo /boot)
conf=$esp/limine.conf

# Keep a limine.conf that predates Misu for uninstall. Taken before the packages
# go in, since their hook writes a fresh one.
if [[ -f $conf && ! -f $conf.pre-misu ]]; then run cp "$conf" "$conf.pre-misu"; fi

install_packages boot

# First run only: copy Limine to the ESP and put it first in the firmware's
# boot order.
if [[ ! -f $esp/EFI/limine/limine_x64.efi ]]; then run limine-install; fi

# Misu's globals replace whatever sits above the first entry; the entries are
# regenerated from the installed kernels and snapshots.
if (( DRY_RUN )); then
  echo "+ set the globals in $esp/limine.conf from $M/boot/limine.conf"
else
  { cat "$M/boot/limine.conf"; echo; [[ -f $conf ]] && sed -n '/^\//,$p' "$conf"; } >"$conf.new"
  mv "$conf.new" "$conf"
fi
run limine-update

# Snapper's root config, written directly: archinstall already mounts the
# @.snapshots subvolume at /.snapshots, which `snapper create-config` refuses.
# Snapshots come from snap-pac only (no hourly timeline); the newest 10 are
# kept. Skipped where / isn't btrfs.
if [[ ! -f /etc/snapper/configs/root && $(findmnt -no FSTYPE /) == btrfs ]]; then
  if (( DRY_RUN )); then echo "+ write /etc/snapper/configs/root and register it"
  else
    [[ -d /.snapshots ]] || btrfs subvolume create /.snapshots
    sed -e 's/^NUMBER_LIMIT=.*/NUMBER_LIMIT="10"/' \
        -e 's/^NUMBER_LIMIT_IMPORTANT=.*/NUMBER_LIMIT_IMPORTANT="5"/' \
        -e 's/^TIMELINE_CREATE=.*/TIMELINE_CREATE="no"/' \
        -e 's/^ALLOW_GROUPS=.*/ALLOW_GROUPS="wheel"/' \
        -e 's/^SYNC_ACL=.*/SYNC_ACL="yes"/' \
        /usr/share/snapper/config-templates/default | install -Dm640 /dev/stdin /etc/snapper/configs/root
    sed -i 's/^SNAPPER_CONFIGS=.*/SNAPPER_CONFIGS="root"/' /etc/conf.d/snapper
    chmod 750 /.snapshots
  fi
fi
if [[ -f /etc/snapper/configs/root || $DRY_RUN == 1 ]]; then
  run systemctl enable --now snapper-cleanup.timer limine-snapper-sync.service
fi
