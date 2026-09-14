#!/bin/sh

set -eu

if ! command -v systemctl >/dev/null 2>&1; then
    exit 0
fi

systemctl daemon-reload || true

case "${1:-}" in
    configure)
        # Debian: an empty second argument identifies a fresh installation.
        if [ -z "${2:-}" ]; then
            systemctl preset procnetd.service || true
            if systemctl is-enabled --quiet procnetd.service; then
                systemctl start procnetd.service || true
            fi
        else
            systemctl try-restart procnetd.service || true
        fi
        ;;
    1)
        # RPM: 1 is a fresh installation.
        systemctl preset procnetd.service || true
        if systemctl is-enabled --quiet procnetd.service; then
            systemctl start procnetd.service || true
        fi
        ;;
    2)
        # RPM: 2 is an upgrade.
        systemctl try-restart procnetd.service || true
        ;;
esac
