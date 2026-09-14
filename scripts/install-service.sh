#!/usr/bin/env bash

# Installs procnetd and procnet with procnetd running as a system service.
#
# - Copies release binaries to /usr/local/bin
# - Installs the systemd unit to /usr/local/lib/systemd/system
# - Reloads, enables and starts the service

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$here/.." && pwd)"

bin_dir="/usr/local/bin"
unit_dir="/usr/local/lib/systemd/system"

unit_src="$repo_root/packaging/procnetd.service"
daemon_src="$repo_root/target/release/procnetd"
client_src="$repo_root/target/release/procnet"

daemon_dst="$bin_dir/procnetd"
client_dst="$bin_dir/procnet"

if [[ ! -x "$daemon_src" || ! -x "$client_src" ]]; then
    echo "Error: release binaries not found, run 'make build-release' first" >&2
    exit 1
fi

if [[ ! -f "$unit_src" ]]; then
    echo "Error: unit file not found at '$unit_src'" >&2
    exit 1
fi

echo "Installing binaries to $bin_dir"
sudo install -D -m 0755 "$daemon_src" "$daemon_dst"
sudo install -m 0755 "$client_src" "$client_dst"

echo ""
echo "Installing systemd unit to $unit_dir"
sudo install -D -m 0644 "$unit_src" "$unit_dir/procnetd.service"

sudo systemctl daemon-reload
sudo systemctl enable procnetd.service
sudo systemctl restart procnetd.service

echo ""
echo "Done. Status:"
systemctl status procnetd.service --no-pager || true
echo ""
echo "Run the TUI with: procnet or $client_dst"
