#!/usr/bin/env bash

set -euo pipefail

prefix="${PREFIX:-$HOME}"
bin_dir="$prefix/.local/bin"
unit_dir="$prefix/.config/systemd/user"

if systemctl --user is-active procnetd.service >/dev/null 2>&1; then
    systemctl --user disable --now procnetd.service
else
    systemctl --user disable procnetd.service 2>/dev/null || true
fi

rm -f "$unit_dir/procnetd.service"
systemctl --user daemon-reload

rm -f "$bin_dir/procnetd" "$bin_dir/procnet"

echo "Done. Removed binaries from $bin_dir and the unit from $unit_dir"
