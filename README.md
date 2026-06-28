# Procnet

A TUI for attributing network usage to processes via eBPF.

## Usage

Enter the development environment and run the daemon and the TUI client in
separate terminals. The daemon only needs a one-time capability grant, not a
running root shell.

```sh
nix develop
make install-caps   # one-time sudo; grants cap_bpf,cap_perfmon,cap_sys_resource
make run-daemon     # run as your normal user
make run-client     # run as your normal user
```

The daemon needs a kernel with BTF available (kernel >= 5.8). Profiling targets
(`stats`, `record`, `flamegraph`, `heaptrack`, `run-profile`,
`run-daemon-profile`) still require `sudo` because they launch an uncapped
profiling build; this keeps `heaptrack` working (it relies on `LD_PRELOAD`).

