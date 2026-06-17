.PHONY: build-release run-release build-profile run-profile \
	stats record flamegraph heaptrack clean

build-release: 
	cargo build --release

run-release: build-release
	sudo ./target/release/procnet

build-profile:
	cargo build --profile profiling

run-profile: build-profile
	sudo ./target/profiling/procnet

stats: build-profile
	sudo perf stat -d ./target/profiling/procnet

record: build-profile
	sudo perf record -g ./target/profiling/procnet

flamegraph: build-profile
	sudo flamegraph -- ./target/profiling/procnet

heaptrack: build-profile
	sudo heaptrack ./target/profiling/procnet

clean:
	cargo clean
