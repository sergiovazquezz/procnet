#!/usr/bin/env bash

set -euo pipefail

bin_dir="/usr/local/bin"
unit_dir="/usr/local/lib/systemd/system"

sudo systemctl disable --now procnetd.service 2>/dev/null || true

sudo rm -f "$unit_dir/procnetd.service"
sudo systemctl daemon-reload
sudo systemctl reset-failed procnetd.service 2>/dev/null || true

sudo rm -f "$bin_dir/procnetd" "$bin_dir/procnet"

echo "Done. Removed binaries from $bin_dir and the unit from $unit_dir"
