#!/usr/bin/env bash
set -euo pipefail

# One-time install of the Linux capabilities the procnetd daemon needs to load
# and attach its eBPF programs without running as root.

binary="${1:-./target/release/procnetd}"

if [[ ! -f "$binary" ]]; then
    echo "error: '$binary' does not exist" >&2
    echo "usage: $0 [path/to/procnetd]" >&2
    exit 1
fi

if [[ ! -x "$binary" ]]; then
    echo "error: '$binary' is not executable" >&2
    exit 1
fi

resolved="$(realpath "$binary")"

echo "Installing capabilities on: $resolved"
sudo setcap 'cap_bpf,cap_perfmon,cap_sys_resource+ep' "$resolved"

echo "Capabilities installed. You can now run the daemon as your normal user:"
echo "  make run-daemon"
echo ""
echo "Verify with: getcap $resolved"
