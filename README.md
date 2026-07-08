# Procnet

A TUI for attributing network usage to processes via eBPF.

The package consists of a Daemon (procnetd) + CLI/TUI (procnet). The Daemon is
recommended to be ran as a user service (as done in the NixOS module and when
using `make install`).

Once the daemon is running a TUI can be attached via the `procnet` binary. There
is no limitation in terms of the number of clients that can be attached to the
daemon.

Use `procnet --help` to get a list of the available commands.

## Requirements

- Linux kernel >= 5.8 with BTF available
- `libbpf` >= 1.0 (raw tracepoint auto-attach)
- `/proc/sys/kernel/perf_event_paranoid` <= 2 (the default on most distros)

## NixOS module

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

## Building From Source

### Dependencies

- Nix:

If you are using Nix you can enter a dev shell with all dependencies from which
you can build the binaries.

```sh
nix develop
```

- Arch Linux:

```sh
sudo pacman -S --needed base-devel rustup clang llvm elfutils libbpf bpf pkgconf zlib
```

### Build

```sh
make install

procnet

make uninstall
```

## Development

Enter the development environment and run the daemon and the TUI client in
separate terminals. The daemon only needs a one-time capability grant, not a
running root shell.

```sh
nix develop         # Or install dependencies for your system

make install-caps   # one-time sudo; grants cap_bpf,cap_perfmon,cap_sys_resource
make run-daemon     # run as your normal user
make run-client     # run as your normal user
```

Profiling targets (`stats`, `record`, `flamegraph`, `heaptrack`, `run-profile`,
`run-daemon-profile`) still require `sudo` because they launch profiling build
with no caps.

The profiling binaries themselves (`perf`, `flamegraph`, `heaptrack`) are not
provided by the dev shell and must be installed on the host.
