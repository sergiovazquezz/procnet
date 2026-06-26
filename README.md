# Procnet

A TUI for attributing network usage to processes via eBPF.

## Usage

Enter the development environment and run the daemon (requires root for eBPF)
and the TUI client in separate terminals:

```sh
nix develop
make run-daemon
make run-client
```

The daemon needs a kernel with BTF available and must run as root; the client
connects to it and renders the TUI.

