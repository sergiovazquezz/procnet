.PHONY: lint test clear-logs build-release run-daemon run-client \
	build-profile run-daemon-profile run-client-profile \
	stats record flamegraph heaptrack clean install-caps \
	install uninstall regen-vmlinux


PID_FILE        := /tmp/procnetd.pid
DAEMON_RELEASE  := ./target/release/procnetd
CLIENT_RELEASE  := ./target/release/procnet
DAEMON_PROFILE  := ./target/profiling/procnetd
CLIENT_PROFILE  := ./target/profiling/procnet
DEV_SOCKET      := $${XDG_RUNTIME_DIR:-/tmp}/procnetd.sock


lint:
	cargo clippy --workspace --all-targets -- -D warnings


test:
	cargo test --workspace


# Release
build-release:
	cargo build --release

run-daemon: build-release install-caps clear-logs
	PROCNET_SOCKET="$(DEV_SOCKET)" $(DAEMON_RELEASE)

run-client: build-release
	PROCNET_SOCKET="$(DEV_SOCKET)" $(CLIENT_RELEASE)


# Profiling
build-profile:
	cargo build --profile profiling

run-daemon-profile: build-profile clear-logs
	sudo env PROCNET_SOCKET="$(DEV_SOCKET)" $(DAEMON_PROFILE)

run-client-profile: build-profile
	PROCNET_SOCKET="$(DEV_SOCKET)" $(CLIENT_PROFILE)

stats: build-profile clear-logs
	sudo env PROCNET_SOCKET="$(DEV_SOCKET)" perf stat -d $(DAEMON_PROFILE)

record: build-profile clear-logs
	sudo env PROCNET_SOCKET="$(DEV_SOCKET)" perf record -g $(DAEMON_PROFILE)

flamegraph: build-profile clear-logs
	sudo env PROCNET_SOCKET="$(DEV_SOCKET)" flamegraph -- $(DAEMON_PROFILE)

heaptrack: build-profile clear-logs
	sudo env PROCNET_SOCKET="$(DEV_SOCKET)" heaptrack $(DAEMON_PROFILE)


# Caps
install-caps: build-release
	@./scripts/install-caps.sh $(DAEMON_RELEASE)


# Service
install: build-release
	@./scripts/install-service.sh

uninstall:
	@./scripts/uninstall-service.sh


# Cleanup
clear-logs:
	@mkdir -p logs
	@-rm -f logs/app.log

clean:
	cargo clean
	@rm -f perf.data perf.data.old flamegraph.svg
	@rm -rf logs


# vmlinux.h
regen-vmlinux:
	@./scripts/regen-vmlinux.sh
