#!/usr/bin/env bash
set -euo pipefail

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

echo ""
echo "Installing capabilities on: $resolved"

sudo setcap 'cap_bpf,cap_perfmon,cap_sys_resource+ep' "$resolved"

installed_caps="$(getcap "$resolved" 2>/dev/null || true)"
if ! printf '%s\n' "$installed_caps" | grep -q 'cap_bpf' \
    || ! printf '%s\n' "$installed_caps" | grep -q 'cap_perfmon' \
    || ! printf '%s\n' "$installed_caps" | grep -q 'cap_sys_resource'; then
    echo "Error: capabilities do not appear to be installed on '$resolved'" >&2
    echo "Verify with: getcap $resolved"
    echo ""
    exit 1
fi

paranoid="$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || echo unknown)"
if [[ "$paranoid" -gt 2 ]] 2>/dev/null; then
    echo "Warning: /proc/sys/kernel/perf_event_paranoid is $paranoid (>2)" >&2
    echo "      The daemon may still need root or a lower value." >&2
fi

echo "Caps installed. You can now run the daemon as your normal user."
