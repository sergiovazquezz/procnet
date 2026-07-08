# Procnet

A TUI for attributing network usage to processes via eBPF.

## Usage

It is recommended to run the daemon as a user service.

```sh
nix develop
make install

procnet

make uninstall
```

## Development

Enter the development environment and run the daemon and the TUI client in
separate terminals. The daemon only needs a one-time capability grant, not a
running root shell.

```sh
nix develop

make install-caps   # one-time sudo; grants cap_bpf,cap_perfmon,cap_sys_resource
make run-daemon     # run as your normal user
make run-client     # run as your normal user
```

## Requirements

- Linux kernel >= 5.8 with BTF available
- `libbpf` >= 1.0 (raw tracepoint auto-attach)
- `/proc/sys/kernel/perf_event_paranoid` <= 2 (the default on most distros)

Profiling targets (`stats`, `record`, `flamegraph`, `heaptrack`, `run-profile`,
`run-daemon-profile`) still require `sudo` because they launch profiling build
with no caps.

The profiling binaries themselves (`perf`, `flamegraph`, `heaptrack`) are not
provided by the Nix devShell and must be installed on the host.

## Build dependencies (non-Nix)

### Arch Linux

```sh
sudo pacman -S --needed base-devel rustup clang llvm elfutils libbpf bpf pkgconf zlib
rustup toolchain install nightly-2026-06-19
rustup default nightly-2026-06-19
```

`bpf` (split from `linux-tools`) provides `bpftool`, used at build time to
generate the kernel's `vmlinux.h` BTF header.

## Installing via Nix

Procnet ships a single package containing both the `procnetd` daemon and the
`procnet` TUI client, plus a NixOS module that wires up capabilities and a
per-user systemd service — the equivalent of `make install` but declarative,
no `sudo setcap` needed.

### As a flake input

Add `procnet` to your flake inputs and import the module:

```nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    procnet = {
      url = "github:sergiovazquezz/procnet";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, procnet, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        procnet.nixosModules.default
        { services.procnet.enable = true; }
      ];
    };
  };
}
```

This puts both binaries on `PATH`, grants `cap_bpf,cap_perfmon,cap_sys_resource`
to `procnetd` via a `security.wrappers` entry, and installs a systemd `--user`
unit `procnetd.service` started per logged-in user. The client connects to
`$XDG_RUNTIME_DIR/procnetd.sock`, matching the existing `--user` model.

A prebuilt binary cache to avoid local rebuilds will be provided separately.
