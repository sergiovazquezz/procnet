#!/usr/bin/env bash

set -euo pipefail

btf_path="/sys/kernel/btf/vmlinux"
if [[ ! -f "$btf_path" ]]; then
    echo "Error: $btf_path not found (need a kernel with CONFIG_DEBUG_INFO_BTF)" >&2
    exit 1
fi

if ! command -v bpftool >/dev/null 2>&1; then
    echo "Error: bpftool not installed" >&2
    exit 1
fi

arch="$(uname -m)"
case "$arch" in
    x86_64)      suffix="x86_64" ;;
    aarch64|arm64) suffix="arm64" ;;
    *)
        echo "Error: unsupported arch '$arch' (expected x86_64 or aarch64)" >&2
        exit 1
        ;;
esac

out="crates/procnetd/src/bpf/vmlinux_${suffix}.h"

echo "Regenerating $out from $btf_path (arch=$arch)..."
bpftool btf dump file "$btf_path" format c > "$out"

echo "Done."
