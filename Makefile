.PHONY: lint clear-logs build-release run-release run-daemon run-client \
	build-profile run-profile run-daemon-profile run-client-profile \
	stats record flamegraph heaptrack clean install-caps

PID_FILE        := /tmp/procnetd.pid
DAEMON_RELEASE  := ./target/release/procnetd
CLIENT_RELEASE  := ./target/release/procnet
DAEMON_PROFILE  := ./target/profiling/procnetd
CLIENT_PROFILE  := ./target/profiling/procnet

lint:
	cargo clippy --all-targets -- -D warnings

clear-logs:
	mkdir -p logs
	-rm -f logs/app.log

build-release:
	cargo build --release

run-release: build-release clear-logs
	@$(DAEMON_RELEASE) & echo $$! > $(PID_FILE); \
	trap 'kill $$(cat $(PID_FILE)) 2>/dev/null; rm -f $(PID_FILE)' EXIT INT; \
	sleep 0.3; \
	$(CLIENT_RELEASE)

run-daemon: build-release clear-logs
	$(DAEMON_RELEASE)

run-client: build-release
	$(CLIENT_RELEASE)

build-profile:
	cargo build --profile profiling

run-profile: build-profile clear-logs
	@sudo $(DAEMON_PROFILE) & echo $$! > $(PID_FILE); \
	trap 'sudo kill $$(cat $(PID_FILE)) 2>/dev/null; rm -f $(PID_FILE)' EXIT INT; \
	sleep 0.3; \
	$(CLIENT_PROFILE)

run-daemon-profile: build-profile clear-logs
	sudo $(DAEMON_PROFILE)

run-client-profile: build-profile
	$(CLIENT_PROFILE)

stats: build-profile clear-logs
	sudo perf stat -d $(DAEMON_PROFILE)

record: build-profile clear-logs
	sudo perf record -g $(DAEMON_PROFILE)

flamegraph: build-profile clear-logs
	sudo flamegraph -- $(DAEMON_PROFILE)

heaptrack: build-profile clear-logs
	sudo heaptrack $(DAEMON_PROFILE)

install-caps: build-release
	sudo setcap 'cap_bpf,cap_perfmon,cap_sys_resource+ep' $(DAEMON_RELEASE)

clean:
	cargo clean
