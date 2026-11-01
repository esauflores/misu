#!/usr/bin/env bash
# Uninstall Misu modules: sudo ./uninstall.sh [--dry-run] [--packages] [module...]
exec "$(dirname -- "${BASH_SOURCE[0]}")/install.sh" --uninstall "$@"
