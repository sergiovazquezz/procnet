#!/usr/bin/env bash

# Installs procnetd and procnet as a systemd --user service.
#
# - Copies release binaries to ~/.local/bin
# - Grants capabilities to procnetd (one-time sudo)
# - Installs the systemd unit to ~/.config/systemd/user
# - Reloads, enables and starts the service

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$here/.." && pwd)"

prefix="${PREFIX:-$HOME}"
bin_dir="$prefix/.local/bin"
unit_dir="$prefix/.config/systemd/user"
unit_src="$repo_root/packaging/procnetd.service"

daemon_src="$repo_root/target/release/procnetd"
client_src="$repo_root/target/release/procnet"
daemon_dst="$bin_dir/procnetd"
client_dst="$bin_dir/procnet"

if [[ ! -x "$daemon_src" || ! -x "$client_src" ]]; then
    echo "Error: release binaries not found; run 'make build-release' first" >&2
    exit 1
fi

if [[ ! -f "$unit_src" ]]; then
    echo "Error: unit file not found at '$unit_src'" >&2
    exit 1
fi

mkdir -p "$bin_dir" "$unit_dir"

echo "Installing binaries to $bin_dir"
install -m 0755 "$daemon_src" "$daemon_dst"
install -m 0755 "$client_src" "$client_dst"

echo ""
echo "Installing capabilities on $daemon_dst (requires sudo)"
sudo setcap 'cap_bpf,cap_perfmon,cap_sys_resource+ep' "$daemon_dst"

echo ""
echo "Installing systemd unit to $unit_dir"
install -m 0644 "$unit_src" "$unit_dir/procnetd.service"

systemctl --user daemon-reload
systemctl --user enable --now procnetd.service

echo ""
echo "Done. Status:"
systemctl --user status procnetd.service --no-pager || true
echo ""
echo "Run the TUI with: procnet or $client_dst"
