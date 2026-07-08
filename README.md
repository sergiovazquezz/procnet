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
