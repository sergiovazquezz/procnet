#!/bin/sh

set -eu

if ! command -v systemctl >/dev/null 2>&1; then
    exit 0
fi

case "${1:-}" in
    0 | remove)
        # Stop and disable only on removal, never while upgrading.
        systemctl disable --now procnetd.service || true
        ;;
esac
