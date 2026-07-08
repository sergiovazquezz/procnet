.PHONY: lint test clear-logs build-release run-release run-daemon run-client \
	build-profile run-profile run-daemon-profile run-client-profile \
	stats record flamegraph heaptrack clean install-caps verify-caps \
	install uninstall


PID_FILE        := /tmp/procnetd.pid
DAEMON_RELEASE  := ./target/release/procnetd
CLIENT_RELEASE  := ./target/release/procnet
DAEMON_PROFILE  := ./target/profiling/procnetd
CLIENT_PROFILE  := ./target/profiling/procnet


lint:
	cargo clippy --workspace --all-targets -- -D warnings


test:
	cargo test --workspace


# Release
build-release:
	cargo build --release

run-daemon: build-release verify-caps clear-logs
	$(DAEMON_RELEASE)

run-client: build-release
	$(CLIENT_RELEASE)


# Profiling
build-profile:
	cargo build --profile profiling

run-daemon-profile: build-profile clear-logs
	sudo $(DAEMON_PROFILE)

run-client-profile: build-profile
	sudo $(CLIENT_PROFILE)

stats: build-profile clear-logs
	sudo perf stat -d $(DAEMON_PROFILE)

record: build-profile clear-logs
	sudo perf record -g $(DAEMON_PROFILE)

flamegraph: build-profile clear-logs
	sudo flamegraph -- $(DAEMON_PROFILE)

heaptrack: build-profile clear-logs
	sudo heaptrack $(DAEMON_PROFILE)


# Caps
install-caps: build-release
	./scripts/install-caps.sh $(DAEMON_RELEASE)

verify-caps: build-release
	@getcap $(DAEMON_RELEASE) | grep -q 'cap_sys_resource,cap_perfmon,cap_bpf=ep' \
	&& echo "" && echo "Success: caps are installed" || $(MAKE) install-caps


# Service
install: build-release
	./scripts/install-service.sh

uninstall:
	./scripts/uninstall-service.sh


# Cleanup
clear-logs:
	mkdir -p logs
	-rm -f logs/app.log

clean:
	cargo clean
	rm -f perf.data perf.data.old flamegraph.svg
	rm -rf logs
