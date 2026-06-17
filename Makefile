.PHONY: clear-logs build-release run-release build-profile run-profile \
	stats record flamegraph heaptrack clean

clear-logs:
	sudo rm logs/app.log

build-release: 
	cargo build --release

run-release: build-release clear-logs
	sudo ./target/release/procnet

build-profile:
	cargo build --profile profiling

run-profile: build-profile clear-logs
	sudo ./target/profiling/procnet

stats: build-profile clear-logs
	sudo perf stat -d ./target/profiling/procnet

record: build-profile clear-logs
	sudo perf record -g ./target/profiling/procnet

flamegraph: build-profile clear-logs
	sudo flamegraph -- ./target/profiling/procnet

heaptrack: build-profile clear-logs
	sudo heaptrack ./target/profiling/procnet

clean:
	cargo clean
